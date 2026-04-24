use super::*;
use soroban_sdk::testutils::Ledger;
use soroban_sdk::{
    testutils::{Address as _, LedgerInfo},
    token, Address, Env,
};

fn create_token_contract<'a>(
    e: &Env,
    admin: &Address,
) -> (token::Client<'a>, token::StellarAssetClient<'a>) {
    let contract = e.register_stellar_asset_contract_v2(admin.clone());
    let contract_address = contract.address();
    (
        token::Client::new(e, &contract_address),
        token::StellarAssetClient::new(e, &contract_address),
    )
}

fn create_escrow_contract<'a>(e: &Env) -> BountyEscrowContractClient<'a> {
    let contract_id = e.register_contract(None, BountyEscrowContract);
    BountyEscrowContractClient::new(e, &contract_id)
}

struct TestSetup<'a> {
    env: Env,
    #[allow(dead_code)]
    admin: Address,
    depositor: Address,
    contributor: Address,
    #[allow(dead_code)]
    token: token::Client<'a>,
    #[allow(dead_code)]
    token_admin: token::StellarAssetClient<'a>,
    escrow: BountyEscrowContractClient<'a>,
}

impl<'a> TestSetup<'a> {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);
        let contributor = Address::generate(&env);

        let (token, token_admin) = create_token_contract(&env, &admin);
        let escrow = create_escrow_contract(&env);

        escrow.init(&admin, &token.address);
        token_admin.mint(&depositor, &1_000_000);

        Self {
            env,
            admin,
            depositor,
            contributor,
            token,
            token_admin,
            escrow,
        }
    }
}

#[test]
fn test_refund_eligibility_ineligible_before_deadline_without_approval() {
    let setup = TestSetup::new();
    let bounty_id = 99;
    let amount = 1_000;
    let deadline = setup.env.ledger().timestamp() + 500;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    let view = setup.escrow.get_refund_eligibility_view(&bounty_id);
    assert!(!view.eligible);
    assert_eq!(
        view.code,
        RefundEligibilityCode::IneligibleDeadlineNotPassed
    );
    assert_eq!(view.amount, 0);
    assert!(!view.approval_present);
}

#[test]
fn test_refund_eligibility_eligible_after_deadline() {
    let setup = TestSetup::new();
    let bounty_id = 100;
    let amount = 1_200;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set_timestamp(deadline + 1);

    let view = setup.escrow.get_refund_eligibility_view(&bounty_id);
    assert!(view.eligible);
    assert_eq!(view.code, RefundEligibilityCode::EligibleDeadlinePassed);
    assert_eq!(view.amount, amount);
    assert_eq!(view.recipient, Some(setup.depositor.clone()));
    assert!(!view.approval_present);
}

#[test]
fn test_refund_eligibility_eligible_with_admin_approval_before_deadline() {
    let setup = TestSetup::new();
    let bounty_id = 101;
    let amount = 2_000;
    let deadline = setup.env.ledger().timestamp() + 1_000;
    let custom_recipient = Address::generate(&setup.env);

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.approve_refund(
        &bounty_id,
        &500,
        &custom_recipient,
        &RefundMode::Partial,
    );

    let view = setup.escrow.get_refund_eligibility_view(&bounty_id);
    assert!(view.eligible);
    assert_eq!(view.code, RefundEligibilityCode::EligibleAdminApproval);
    assert_eq!(view.amount, 500);
    assert_eq!(view.recipient, Some(custom_recipient));
    assert!(view.approval_present);
}

// Valid transitions: Locked → Released
#[test]
fn test_locked_to_released() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Locked
    );

    setup.escrow.release_funds(&bounty_id, &setup.contributor);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Released
    );
}

// Valid transitions: Locked → Refunded
#[test]
fn test_locked_to_refunded() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Locked
    );

    setup.env.ledger().set_timestamp(deadline + 1);
    setup.escrow.refund(&bounty_id);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Refunded
    );
}

// Valid transitions: Locked → PartiallyRefunded
#[test]
fn test_locked_to_partially_refunded() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Locked
    );

    // Approve partial refund before deadline
    setup
        .escrow
        .approve_refund(&bounty_id, &500, &setup.depositor, &RefundMode::Partial);
    setup.escrow.refund(&bounty_id);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::PartiallyRefunded
    );
}

// Valid transitions: PartiallyRefunded → Refunded
#[test]
fn test_partially_refunded_to_refunded() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    // First partial refund
    setup
        .escrow
        .approve_refund(&bounty_id, &500, &setup.depositor, &RefundMode::Partial);
    setup.escrow.refund(&bounty_id);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::PartiallyRefunded
    );

    // Second refund completes it
    setup.env.ledger().set_timestamp(deadline + 1);
    setup.escrow.refund(&bounty_id);
    assert_eq!(
        setup.escrow.get_escrow_info(&bounty_id).status,
        EscrowStatus::Refunded
    );
}

// Invalid transition: Released → Locked
#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_released_to_locked_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.release_funds(&bounty_id, &setup.contributor);

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
}

// Invalid transition: Released → Released
#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_released_to_released_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.release_funds(&bounty_id, &setup.contributor);

    setup.escrow.release_funds(&bounty_id, &setup.contributor);
}

// Invalid transition: Released → Refunded
#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_released_to_refunded_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.release_funds(&bounty_id, &setup.contributor);

    setup.env.ledger().set_timestamp(deadline + 1);
    setup.escrow.refund(&bounty_id);
}

// Invalid transition: Released → PartiallyRefunded
#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_released_to_partially_refunded_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.release_funds(&bounty_id, &setup.contributor);

    setup.env.ledger().set_timestamp(deadline + 1);
    setup
        .escrow
        .partial_release(&bounty_id, &setup.contributor, &500);
}

// Invalid transition: Refunded → Locked
#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_refunded_to_locked_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set(LedgerInfo {
        timestamp: deadline + 1,
        protocol_version: 20,
        sequence_number: 0,
        network_id: Default::default(),
        base_reserve: 0,
        min_temp_entry_ttl: 0,
        min_persistent_entry_ttl: 0,
        max_entry_ttl: 0,
    });
    setup.escrow.refund(&bounty_id);

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
}

// Invalid transition: Refunded → Released
#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_refunded_to_released_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set(LedgerInfo {
        timestamp: deadline + 1,
        protocol_version: 20,
        sequence_number: 0,
        network_id: Default::default(),
        base_reserve: 0,
        min_temp_entry_ttl: 0,
        min_persistent_entry_ttl: 0,
        max_entry_ttl: 0,
    });
    setup.escrow.refund(&bounty_id);

    setup.escrow.release_funds(&bounty_id, &setup.contributor);
}

// Invalid transition: Refunded → Refunded
#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_refunded_to_refunded_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set(LedgerInfo {
        timestamp: deadline + 1,
        protocol_version: 20,
        sequence_number: 0,
        network_id: Default::default(),
        base_reserve: 0,
        min_temp_entry_ttl: 0,
        min_persistent_entry_ttl: 0,
        max_entry_ttl: 0,
    });
    setup.escrow.refund(&bounty_id);

    setup.escrow.refund(&bounty_id);
}

// Invalid transition: Refunded → PartiallyRefunded
#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_refunded_to_partially_refunded_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set(LedgerInfo {
        timestamp: deadline + 1,
        protocol_version: 20,
        sequence_number: 0,
        network_id: Default::default(),
        base_reserve: 0,
        min_temp_entry_ttl: 0,
        min_persistent_entry_ttl: 0,
        max_entry_ttl: 0,
    });
    setup.escrow.refund(&bounty_id);

    setup
        .escrow
        .partial_release(&bounty_id, &setup.contributor, &100);
}

// Invalid transition: PartiallyRefunded → Locked
#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_partially_refunded_to_locked_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup
        .escrow
        .approve_refund(&bounty_id, &500, &setup.depositor, &RefundMode::Partial);
    setup.escrow.refund(&bounty_id);

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
}

// Invalid transition: PartiallyRefunded → Released
#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_partially_refunded_to_released_fails() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 100;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup
        .escrow
        .approve_refund(&bounty_id, &500, &setup.depositor, &RefundMode::Partial);
    setup.escrow.refund(&bounty_id);

    setup.escrow.release_funds(&bounty_id, &setup.contributor);
}

// ============================================================================
// RISK FLAGS GOVERNANCE TESTS (Issue #36)
// ============================================================================
//
// These tests verify the risk-flags governance invariants:
//   - set_escrow_risk_flags ORs bits into the stored flags
//   - clear_escrow_risk_flags ANDs-NOT bits from the stored flags
//   - Reserved bits outside RISK_FLAGS_VALID_MASK are rejected
//   - RiskFlagsUpdated audit event is emitted on every change
//   - Upgrade-safe schema version is written on init
//   - update_metadata preserves existing risk flags
//   - get_metadata returns default (zero) when no metadata exists

/// RF-1: set_escrow_risk_flags ORs bits into stored flags.
#[test]
fn test_risk_flags_set_ors_bits() {
    let setup = TestSetup::new();
    let bounty_id = 200u64;

    let meta = setup
        .escrow
        .set_escrow_risk_flags(&bounty_id, &RISK_FLAG_HIGH_RISK)
        .unwrap();
    assert_eq!(meta.risk_flags, RISK_FLAG_HIGH_RISK);

    let meta2 = setup
        .escrow
        .set_escrow_risk_flags(&bounty_id, &RISK_FLAG_UNDER_REVIEW)
        .unwrap();
    assert_eq!(
        meta2.risk_flags,
        RISK_FLAG_HIGH_RISK | RISK_FLAG_UNDER_REVIEW
    );
}

/// RF-2: clear_escrow_risk_flags ANDs-NOT bits from stored flags.
#[test]
fn test_risk_flags_clear_removes_bits() {
    let setup = TestSetup::new();
    let bounty_id = 201u64;
    let all = RISK_FLAG_HIGH_RISK | RISK_FLAG_UNDER_REVIEW | RISK_FLAG_RESTRICTED;

    setup.escrow.set_escrow_risk_flags(&bounty_id, &all).unwrap();
    let meta = setup
        .escrow
        .clear_escrow_risk_flags(&bounty_id, &RISK_FLAG_UNDER_REVIEW)
        .unwrap();
    assert_eq!(meta.risk_flags, RISK_FLAG_HIGH_RISK | RISK_FLAG_RESTRICTED);
}

/// RF-3: clear is idempotent — clearing already-cleared bits is a no-op.
#[test]
fn test_risk_flags_clear_idempotent() {
    let setup = TestSetup::new();
    let bounty_id = 202u64;

    setup
        .escrow
        .set_escrow_risk_flags(&bounty_id, &RISK_FLAG_HIGH_RISK)
        .unwrap();
    // Clear a bit that was never set.
    let meta = setup
        .escrow
        .clear_escrow_risk_flags(&bounty_id, &RISK_FLAG_UNDER_REVIEW)
        .unwrap();
    assert_eq!(meta.risk_flags, RISK_FLAG_HIGH_RISK);
}

/// RF-4: all valid bits can be set and cleared together.
#[test]
fn test_risk_flags_all_valid_bits_round_trip() {
    let setup = TestSetup::new();
    let bounty_id = 203u64;
    let all = RISK_FLAG_HIGH_RISK | RISK_FLAG_UNDER_REVIEW | RISK_FLAG_RESTRICTED | RISK_FLAG_DEPRECATED;

    setup.escrow.set_escrow_risk_flags(&bounty_id, &all).unwrap();
    assert_eq!(setup.escrow.get_metadata(&bounty_id).risk_flags, all);

    setup.escrow.clear_escrow_risk_flags(&bounty_id, &all).unwrap();
    assert_eq!(setup.escrow.get_metadata(&bounty_id).risk_flags, 0);
}

/// RF-5: reserved bits are rejected by set_escrow_risk_flags.
#[test]
fn test_risk_flags_reserved_bits_rejected_on_set() {
    let setup = TestSetup::new();
    let bounty_id = 204u64;
    let reserved = 1u32 << 31; // not in RISK_FLAGS_VALID_MASK
    let result = setup.escrow.try_set_escrow_risk_flags(&bounty_id, &reserved);
    assert!(result.is_err(), "reserved bits must be rejected");
}

/// RF-6: reserved bits are rejected by clear_escrow_risk_flags.
#[test]
fn test_risk_flags_reserved_bits_rejected_on_clear() {
    let setup = TestSetup::new();
    let bounty_id = 205u64;
    let reserved = 1u32 << 16;
    let result = setup.escrow.try_clear_escrow_risk_flags(&bounty_id, &reserved);
    assert!(result.is_err(), "reserved bits must be rejected on clear");
}

/// RF-7: RiskFlagsUpdated audit event is emitted on set.
#[test]
fn test_risk_flags_set_emits_audit_event() {
    let setup = TestSetup::new();
    let bounty_id = 206u64;
    let before = setup.env.events().all().len();
    setup
        .escrow
        .set_escrow_risk_flags(&bounty_id, &RISK_FLAG_HIGH_RISK)
        .unwrap();
    assert!(
        setup.env.events().all().len() > before,
        "RiskFlagsUpdated must be emitted on set"
    );
}

/// RF-8: RiskFlagsUpdated audit event is emitted on clear.
#[test]
fn test_risk_flags_clear_emits_audit_event() {
    let setup = TestSetup::new();
    let bounty_id = 207u64;
    setup
        .escrow
        .set_escrow_risk_flags(&bounty_id, &RISK_FLAG_HIGH_RISK)
        .unwrap();
    let before = setup.env.events().all().len();
    setup
        .escrow
        .clear_escrow_risk_flags(&bounty_id, &RISK_FLAG_HIGH_RISK)
        .unwrap();
    assert!(
        setup.env.events().all().len() > before,
        "RiskFlagsUpdated must be emitted on clear"
    );
}

/// RF-9: upgrade-safe schema version is written on init.
#[test]
fn test_risk_flags_schema_version_written_on_init() {
    let setup = TestSetup::new();
    let version = setup.escrow.get_risk_flags_schema_version();
    assert_eq!(version, 1u32, "schema version must be 1 after init");
}

/// RF-10: get_metadata returns default (zero flags) when no metadata exists.
#[test]
fn test_risk_flags_get_metadata_default_is_zero() {
    let setup = TestSetup::new();
    let meta = setup.escrow.get_metadata(&999u64);
    assert_eq!(meta.risk_flags, 0, "default risk_flags must be 0");
}

/// RF-11: update_metadata preserves existing risk flags.
#[test]
fn test_risk_flags_preserved_across_metadata_update() {
    let setup = TestSetup::new();
    let bounty_id = 208u64;

    setup
        .escrow
        .set_escrow_risk_flags(&bounty_id, &(RISK_FLAG_HIGH_RISK | RISK_FLAG_RESTRICTED))
        .unwrap();

    setup
        .escrow
        .update_metadata(
            &setup.admin,
            &bounty_id,
            &42u64,
            &100u64,
            &soroban_sdk::String::from_str(&setup.env, "bug_fix"),
            &None,
        )
        .unwrap();

    let meta = setup.escrow.get_metadata(&bounty_id);
    assert_eq!(
        meta.risk_flags,
        RISK_FLAG_HIGH_RISK | RISK_FLAG_RESTRICTED,
        "risk flags must survive metadata update"
    );
    assert_eq!(meta.repo_id, 42u64);
    assert_eq!(meta.issue_id, 100u64);
}

/// RF-12: set_escrow_risk_flags requires contract to be initialized.
#[test]
fn test_risk_flags_requires_init() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, BountyEscrowContract);
    let client = BountyEscrowContractClient::new(&env, &contract_id);
    let result = client.try_set_escrow_risk_flags(&1u64, &RISK_FLAG_HIGH_RISK);
    assert!(result.is_err(), "must fail when not initialized");
}

/// RF-13: multiple set calls accumulate flags correctly.
#[test]
fn test_risk_flags_multiple_sets_accumulate() {
    let setup = TestSetup::new();
    let bounty_id = 209u64;

    setup
        .escrow
        .set_escrow_risk_flags(&bounty_id, &RISK_FLAG_HIGH_RISK)
        .unwrap();
    setup
        .escrow
        .set_escrow_risk_flags(&bounty_id, &RISK_FLAG_RESTRICTED)
        .unwrap();
    setup
        .escrow
        .set_escrow_risk_flags(&bounty_id, &RISK_FLAG_DEPRECATED)
        .unwrap();

    let meta = setup.escrow.get_metadata(&bounty_id);
    assert_eq!(
        meta.risk_flags,
        RISK_FLAG_HIGH_RISK | RISK_FLAG_RESTRICTED | RISK_FLAG_DEPRECATED
    );
}

/// RF-14: zero flags value is accepted (no-op set).
#[test]
fn test_risk_flags_zero_value_accepted() {
    let setup = TestSetup::new();
    let bounty_id = 210u64;
    setup
        .escrow
        .set_escrow_risk_flags(&bounty_id, &RISK_FLAG_HIGH_RISK)
        .unwrap();
    // Setting zero is a no-op but must not error.
    let meta = setup
        .escrow
        .set_escrow_risk_flags(&bounty_id, &0u32)
        .unwrap();
    assert_eq!(meta.risk_flags, RISK_FLAG_HIGH_RISK);
}

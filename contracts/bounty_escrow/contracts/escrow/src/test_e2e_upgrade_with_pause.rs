//! End-to-end pause/upgrade/resume style tests.
//!
//! These tests simulate upgrade windows by pausing operations, verifying
//! escrow state/fund safety, then resuming.

#![cfg(test)]

use crate::{BountyEscrowContract, BountyEscrowContractClient, Error, EscrowStatus};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    token, Address, Env, String as SorobanString,
};

struct TestContext<'a> {
    env: Env,
    client: BountyEscrowContractClient<'a>,
    token_client: token::Client<'a>,
    token_admin_client: token::StellarAssetClient<'a>,
    depositor: Address,
    contributor: Address,
}

impl<'a> TestContext<'a> {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, BountyEscrowContract);
        let client = BountyEscrowContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);
        let contributor = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
        let token_addr = token_contract.address();
        let token_client = token::Client::new(&env, &token_addr);
        let token_admin_client = token::StellarAssetClient::new(&env, &token_addr);

        client.init(&admin, &token_addr);
        token_admin_client.mint(&depositor, &2_000_000_000);

        Self {
            env,
            client,
            token_client,
            token_admin_client,
            depositor,
            contributor,
        }
    }

    fn lock_bounty(&self, bounty_id: u64, amount: i128) {
        let deadline = self.env.ledger().timestamp() + 86_400;
        self.client
            .lock_funds(&self.depositor, &bounty_id, &amount, &deadline);
    }

    fn contract_balance(&self) -> i128 {
        self.token_client.balance(&self.client.address)
    }
}

#[test]
fn test_e2e_pause_upgrade_resume_with_funds() {
    let ctx = TestContext::new();
    let bounty_id = 1u64;
    let amount = 10_000i128;

    ctx.lock_bounty(bounty_id, amount);
    assert_eq!(ctx.contract_balance(), amount);

    ctx.client.set_paused(
        &Some(true),
        &Some(true),
        &Some(true),
        &Some(SorobanString::from_str(&ctx.env, "Upgrade in progress")),
    );

    let flags = ctx.client.get_pause_flags();
    assert!(flags.lock_paused);
    assert!(flags.release_paused);
    assert!(flags.refund_paused);

    let lock_err = ctx.client.try_lock_funds(
        &ctx.depositor,
        &2u64,
        &5_000i128,
        &(ctx.env.ledger().timestamp() + 86_400),
    );
    assert!(lock_err.is_err());

    let rel_err = ctx.client.try_release_funds(&bounty_id, &ctx.contributor);
    assert!(rel_err.is_err());

    ctx.client
        .set_paused(&Some(false), &Some(false), &Some(false), &None);
    let flags_after = ctx.client.get_pause_flags();
    assert!(!flags_after.lock_paused);
    assert!(!flags_after.release_paused);
    assert!(!flags_after.refund_paused);

    ctx.client.release_funds(&bounty_id, &ctx.contributor);
    let escrow = ctx.client.get_escrow_info(&bounty_id);
    assert_eq!(escrow.status, EscrowStatus::Released);
    assert_eq!(ctx.contract_balance(), 0);
    assert_eq!(ctx.token_client.balance(&ctx.contributor), amount);
}

#[test]
fn test_e2e_upgrade_with_multiple_bounties() {
    let ctx = TestContext::new();
    let bounties = [(1u64, 10_000i128), (2u64, 20_000i128), (3u64, 15_000i128)];

    let mut total_locked = 0i128;
    for (id, amount) in bounties {
        ctx.lock_bounty(id, amount);
        total_locked += amount;
    }
    assert_eq!(ctx.contract_balance(), total_locked);

    ctx.client
        .set_paused(&Some(true), &Some(true), &Some(true), &None);

    for (id, amount) in bounties {
        let escrow = ctx.client.get_escrow_info(&id);
        assert_eq!(escrow.status, EscrowStatus::Locked);
        assert_eq!(escrow.amount, amount);
    }

    ctx.client
        .set_paused(&Some(false), &Some(false), &Some(false), &None);
    assert_eq!(ctx.contract_balance(), total_locked);
}

#[test]
fn test_e2e_emergency_withdraw_requires_pause() {
    let ctx = TestContext::new();
    ctx.lock_bounty(1, 10_000);

    let target = Address::generate(&ctx.env);
    let err = ctx.client.try_emergency_withdraw(&target);
    assert_eq!(err, Err(Ok(Error::NotPaused)));

    ctx.client.set_paused(&Some(true), &None, &None, &None);
    ctx.client.emergency_withdraw(&target);

    assert_eq!(ctx.contract_balance(), 0);
    assert_eq!(ctx.token_client.balance(&target), 10_000);
}

#[test]
fn test_e2e_selective_pause_during_upgrade() {
    let ctx = TestContext::new();
    ctx.lock_bounty(1, 10_000);

    ctx.client
        .set_paused(&Some(true), &Some(false), &Some(false), &None);

    let lock_result = ctx.client.try_lock_funds(
        &ctx.depositor,
        &2u64,
        &5_000i128,
        &(ctx.env.ledger().timestamp() + 86_400),
    );
    assert!(lock_result.is_err());

    ctx.client.release_funds(&1, &ctx.contributor);
    let escrow = ctx.client.get_escrow_info(&1);
    assert_eq!(escrow.status, EscrowStatus::Released);
}

#[test]
fn test_e2e_upgrade_cycle_emits_events() {
    let ctx = TestContext::new();
    ctx.lock_bounty(1, 10_000);

    let events_before_pause = ctx.env.events().all().len();

    ctx.client.set_paused(
        &Some(true),
        &Some(true),
        &Some(true),
        &Some(SorobanString::from_str(&ctx.env, "Maintenance")),
    );
    let events_after_pause = ctx.env.events().all().len();
    assert!(events_after_pause > events_before_pause);

    ctx.client
        .set_paused(&Some(false), &Some(false), &Some(false), &None);
    let events_after_resume = ctx.env.events().all().len();
    assert!(events_after_resume > events_after_pause);
}

#[test]
fn test_e2e_upgrade_with_high_value_bounties() {
    let ctx = TestContext::new();
    let high_value = 100_000_000i128;

    ctx.token_admin_client
        .mint(&ctx.depositor, &(high_value * 3i128));
    ctx.lock_bounty(11, high_value);
    ctx.lock_bounty(12, high_value);
    ctx.lock_bounty(13, high_value);

    let total = high_value * 3;
    assert_eq!(ctx.contract_balance(), total);

    ctx.client
        .set_paused(&Some(true), &Some(true), &Some(true), &None);
    assert_eq!(ctx.contract_balance(), total);

    ctx.client
        .set_paused(&Some(false), &Some(false), &Some(false), &None);
    assert_eq!(ctx.contract_balance(), total);
}

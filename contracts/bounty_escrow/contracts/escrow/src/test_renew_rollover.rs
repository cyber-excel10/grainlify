//! Renewal and rollover lifecycle tests.
//!
//! Focus:
//! - renew extends deadlines without losing locked funds
//! - optional top-up preserves accounting invariants
//! - rollover creates explicit previous/next cycle links
//! - invalid state transitions are rejected deterministically

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, token, Address, Env};

struct RenewTestSetup<'a> {
    env: Env,
    depositor: Address,
    contributor: Address,
    token: token::Client<'a>,
    token_admin: token::StellarAssetClient<'a>,
    escrow: BountyEscrowContractClient<'a>,
}

impl<'a> RenewTestSetup<'a> {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);
        let contributor = Address::generate(&env);
        let token_admin_addr = Address::generate(&env);

        let token_id = env
            .register_stellar_asset_contract_v2(token_admin_addr.clone())
            .address();
        let token = token::Client::new(&env, &token_id);
        let token_admin = token::StellarAssetClient::new(&env, &token_id);

        let contract_id = env.register_contract(None, BountyEscrowContract);
        let escrow = BountyEscrowContractClient::new(&env, &contract_id);
        escrow.init(&admin, &token_id);

        token_admin.mint(&depositor, &10_000_000);

        Self {
            env,
            depositor,
            contributor,
            token,
            token_admin,
            escrow,
        }
    }

    fn lock_bounty(&self, bounty_id: u64, amount: i128, deadline: u64) {
        self.escrow
            .lock_funds(&self.depositor, &bounty_id, &amount, &deadline);
    }
}

#[test]
fn test_renew_extends_deadline_without_losing_funds() {
    let s = RenewTestSetup::new();
    let bounty_id = 100_u64;
    let amount = 5_000_i128;
    let initial_deadline = s.env.ledger().timestamp() + 1_000;

    s.lock_bounty(bounty_id, amount, initial_deadline);

    let contract_balance_before = s.token.balance(&s.escrow.address);
    let depositor_balance_before = s.token.balance(&s.depositor);

    let new_deadline = initial_deadline + 2_000;
    s.escrow.renew_escrow(&bounty_id, &new_deadline, &0_i128);

    let escrow = s.escrow.get_escrow_info(&bounty_id);
    assert_eq!(escrow.deadline, new_deadline);
    assert_eq!(escrow.amount, amount);
    assert_eq!(escrow.remaining_amount, amount);
    assert_eq!(escrow.status, EscrowStatus::Locked);
    assert_eq!(s.token.balance(&s.escrow.address), contract_balance_before);
    assert_eq!(s.token.balance(&s.depositor), depositor_balance_before);
}

#[test]
fn test_renew_with_topup_increases_locked_balance_and_amount() {
    let s = RenewTestSetup::new();
    let bounty_id = 101_u64;
    let amount = 5_000_i128;
    let topup = 3_000_i128;
    let initial_deadline = s.env.ledger().timestamp() + 1_000;

    s.lock_bounty(bounty_id, amount, initial_deadline);

    let contract_balance_before = s.token.balance(&s.escrow.address);
    let depositor_balance_before = s.token.balance(&s.depositor);

    s.escrow
        .renew_escrow(&bounty_id, &(initial_deadline + 2_000), &topup);

    let escrow = s.escrow.get_escrow_info(&bounty_id);
    assert_eq!(escrow.amount, amount + topup);
    assert_eq!(escrow.remaining_amount, amount + topup);
    assert_eq!(
        s.token.balance(&s.escrow.address),
        contract_balance_before + topup
    );
    assert_eq!(
        s.token.balance(&s.depositor),
        depositor_balance_before - topup
    );
}

#[test]
fn test_renew_history_is_append_only_and_ordered() {
    let s = RenewTestSetup::new();
    let bounty_id = 102_u64;
    let d1 = s.env.ledger().timestamp() + 1_000;

    s.lock_bounty(bounty_id, 5_000, d1);

    let d2 = d1 + 2_000;
    let d3 = d2 + 2_000;

    s.escrow.renew_escrow(&bounty_id, &d2, &1_000);
    s.escrow.renew_escrow(&bounty_id, &d3, &500);

    let history = s.escrow.get_renewal_history(&bounty_id);
    assert_eq!(history.len(), 2);

    let r1 = history.get(0).unwrap();
    assert_eq!(r1.cycle, 1);
    assert_eq!(r1.old_deadline, d1);
    assert_eq!(r1.new_deadline, d2);
    assert_eq!(r1.additional_amount, 1_000);

    let r2 = history.get(1).unwrap();
    assert_eq!(r2.cycle, 2);
    assert_eq!(r2.old_deadline, d2);
    assert_eq!(r2.new_deadline, d3);
    assert_eq!(r2.additional_amount, 500);
}

#[test]
fn test_renew_rejects_invalid_transitions() {
    let s = RenewTestSetup::new();
    let bounty_id = 103_u64;
    let amount = 5_000_i128;
    let deadline = s.env.ledger().timestamp() + 1_000;

    s.lock_bounty(bounty_id, amount, deadline);

    let same_deadline = s.escrow.try_renew_escrow(&bounty_id, &deadline, &0);
    assert_eq!(same_deadline, Err(Ok(Error::InvalidDeadline)));

    let negative = s
        .escrow
        .try_renew_escrow(&bounty_id, &(deadline + 1_000), &-1);
    assert_eq!(negative, Err(Ok(Error::InvalidAmount)));

    s.escrow.release_funds(&bounty_id, &s.contributor);
    let released = s
        .escrow
        .try_renew_escrow(&bounty_id, &(deadline + 2_000), &0);
    assert_eq!(released, Err(Ok(Error::FundsNotLocked)));
}

#[test]
fn test_renew_rejects_when_already_expired() {
    let s = RenewTestSetup::new();
    let bounty_id = 104_u64;
    let amount = 5_000_i128;
    let deadline = s.env.ledger().timestamp() + 100;

    s.lock_bounty(bounty_id, amount, deadline);
    s.env.ledger().set_timestamp(deadline + 1);

    let res = s
        .escrow
        .try_renew_escrow(&bounty_id, &(deadline + 2_000), &0_i128);
    assert_eq!(res, Err(Ok(Error::DeadlineNotPassed)));
}

#[test]
fn test_renew_nonexistent_fails() {
    let s = RenewTestSetup::new();
    let res = s.escrow.try_renew_escrow(&999_u64, &10_000, &0);
    assert_eq!(res, Err(Ok(Error::BountyNotFound)));
}

#[test]
fn test_create_next_cycle_after_release_links_chain() {
    let s = RenewTestSetup::new();
    let first = 200_u64;
    let second = 201_u64;
    let amount = 5_000_i128;
    let d1 = s.env.ledger().timestamp() + 1_000;
    let d2 = s.env.ledger().timestamp() + 5_000;

    s.lock_bounty(first, amount, d1);
    s.escrow.release_funds(&first, &s.contributor);

    let contract_before = s.token.balance(&s.escrow.address);
    let depositor_before = s.token.balance(&s.depositor);

    s.escrow.create_next_cycle(&first, &second, &amount, &d2);

    let new_escrow = s.escrow.get_escrow_info(&second);
    assert_eq!(new_escrow.status, EscrowStatus::Locked);
    assert_eq!(new_escrow.amount, amount);
    assert_eq!(new_escrow.remaining_amount, amount);
    assert_eq!(new_escrow.deadline, d2);
    assert_eq!(new_escrow.depositor, s.depositor);

    let link_first = s.escrow.get_cycle_info(&first);
    assert_eq!(link_first.previous_id, 0);
    assert_eq!(link_first.next_id, second);

    let link_second = s.escrow.get_cycle_info(&second);
    assert_eq!(link_second.previous_id, first);
    assert_eq!(link_second.next_id, 0);
    assert_eq!(link_second.cycle, 1);

    assert_eq!(s.token.balance(&s.escrow.address), contract_before + amount);
    assert_eq!(s.token.balance(&s.depositor), depositor_before - amount);
}

#[test]
fn test_create_next_cycle_after_refund_is_allowed() {
    let s = RenewTestSetup::new();
    let first = 210_u64;
    let second = 211_u64;
    let amount = 5_000_i128;
    let deadline = s.env.ledger().timestamp() + 100;

    s.lock_bounty(first, amount, deadline);
    s.env.ledger().set_timestamp(deadline + 1);
    s.escrow.refund(&first);

    s.escrow.create_next_cycle(
        &first,
        &second,
        &amount,
        &(s.env.ledger().timestamp() + 10_000),
    );

    let link_first = s.escrow.get_cycle_info(&first);
    let link_second = s.escrow.get_cycle_info(&second);
    assert_eq!(link_first.next_id, second);
    assert_eq!(link_second.previous_id, first);
}

#[test]
fn test_create_next_cycle_rejects_invalid_state_and_duplicate_successor() {
    let s = RenewTestSetup::new();
    let id1 = 300_u64;
    let id2 = 301_u64;
    let id3 = 302_u64;
    let amount = 3_000_i128;
    let d1 = s.env.ledger().timestamp() + 1_000;
    let d2 = s.env.ledger().timestamp() + 5_000;

    s.lock_bounty(id1, amount, d1);

    let while_locked = s.escrow.try_create_next_cycle(&id1, &id2, &amount, &d2);
    assert_eq!(while_locked, Err(Ok(Error::FundsNotLocked)));

    s.escrow.release_funds(&id1, &s.contributor);
    s.escrow.create_next_cycle(&id1, &id2, &amount, &d2);

    let dup_successor = s
        .escrow
        .try_create_next_cycle(&id1, &id3, &amount, &(d2 + 1_000));
    assert_eq!(dup_successor, Err(Ok(Error::BountyExists)));
}

#[test]
fn test_create_next_cycle_rejects_invalid_params() {
    let s = RenewTestSetup::new();
    let id1 = 400_u64;
    let id2 = 401_u64;
    let amount = 3_000_i128;
    let d1 = s.env.ledger().timestamp() + 1_000;

    s.lock_bounty(id1, amount, d1);
    s.escrow.release_funds(&id1, &s.contributor);

    let zero_amount =
        s.escrow
            .try_create_next_cycle(&id1, &id2, &0, &(s.env.ledger().timestamp() + 5_000));
    assert_eq!(zero_amount, Err(Ok(Error::InvalidAmount)));

    let past_deadline =
        s.escrow
            .try_create_next_cycle(&id1, &id2, &amount, &(s.env.ledger().timestamp()));
    assert_eq!(past_deadline, Err(Ok(Error::InvalidDeadline)));

    let same_id =
        s.escrow
            .try_create_next_cycle(&id1, &id1, &amount, &(s.env.ledger().timestamp() + 5_000));
    assert_eq!(same_id, Err(Ok(Error::BountyExists)));
}

#[test]
fn test_cycle_info_defaults_and_not_found_paths() {
    let s = RenewTestSetup::new();
    let bounty_id = 500_u64;
    s.lock_bounty(bounty_id, 1_000, s.env.ledger().timestamp() + 1_000);

    let default_link = s.escrow.get_cycle_info(&bounty_id);
    assert_eq!(default_link.previous_id, 0);
    assert_eq!(default_link.next_id, 0);
    assert_eq!(default_link.cycle, 1);

    let empty_history = s.escrow.get_renewal_history(&bounty_id);
    assert_eq!(empty_history.len(), 0);

    assert_eq!(
        s.escrow.try_get_cycle_info(&999_u64),
        Err(Ok(Error::BountyNotFound))
    );
    assert_eq!(
        s.escrow.try_get_renewal_history(&999_u64),
        Err(Ok(Error::BountyNotFound))
    );
}

#[test]
fn test_rollover_preserves_prior_renewal_history() {
    let s = RenewTestSetup::new();
    let first = 600_u64;
    let second = 601_u64;
    let d1 = s.env.ledger().timestamp() + 1_000;
    let d2 = d1 + 1_000;
    let d3 = d2 + 1_000;

    s.lock_bounty(first, 5_000, d1);
    s.escrow.renew_escrow(&first, &d2, &0);
    s.escrow.renew_escrow(&first, &d3, &2_000);

    let before = s.escrow.get_renewal_history(&first);
    assert_eq!(before.len(), 2);

    s.escrow.release_funds(&first, &s.contributor);
    s.escrow.create_next_cycle(
        &first,
        &second,
        &5_000,
        &(s.env.ledger().timestamp() + 10_000),
    );

    let after = s.escrow.get_renewal_history(&first);
    assert_eq!(after.len(), 2);
    assert_eq!(after.get(0).unwrap(), before.get(0).unwrap());
    assert_eq!(after.get(1).unwrap(), before.get(1).unwrap());

    let link_second = s.escrow.get_cycle_info(&second);
    assert_eq!(link_second.previous_id, first);
}

#[test]
fn test_rollover_can_chain_three_cycles() {
    let s = RenewTestSetup::new();
    let id1 = 700_u64;
    let id2 = 701_u64;
    let id3 = 702_u64;
    let amount = 2_000_i128;
    let base = s.env.ledger().timestamp();

    s.lock_bounty(id1, amount, base + 1_000);
    s.escrow.release_funds(&id1, &s.contributor);

    s.escrow
        .create_next_cycle(&id1, &id2, &amount, &(base + 2_000));
    s.escrow.release_funds(&id2, &s.contributor);

    s.escrow
        .create_next_cycle(&id2, &id3, &amount, &(base + 3_000));

    let l1 = s.escrow.get_cycle_info(&id1);
    let l2 = s.escrow.get_cycle_info(&id2);
    let l3 = s.escrow.get_cycle_info(&id3);

    assert_eq!(l1.previous_id, 0);
    assert_eq!(l1.next_id, id2);

    assert_eq!(l2.previous_id, id1);
    assert_eq!(l2.next_id, id3);
    assert_eq!(l2.cycle, 1);

    assert_eq!(l3.previous_id, id2);
    assert_eq!(l3.next_id, 0);
    assert_eq!(l3.cycle, 2);
}

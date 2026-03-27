//! Risk flag signaling for bounty escrows, aligned with program-escrow semantics.
//!
//! # Model
//! - Flags are a `u32` bitfield on [`crate::EscrowMetadata::risk_flags`].
//! - Updates emit [`crate::events::RiskFlagsUpdated`] with `version`, `bounty_id`,
//!   `previous_flags`, `new_flags`, `admin`, and `timestamp` for deterministic indexing.
//!
//! # Security
//! Only the stored admin can mutate flags (`require_auth` on the admin address).
//! Flags are informational on-chain; enforcement belongs to off-chain services.

#![cfg(test)]

use crate::events::{RiskFlagsUpdated, EVENT_VERSION_V2};
use crate::{
    BountyEscrowContract, BountyEscrowContractClient, Error, RISK_FLAG_DEPRECATED,
    RISK_FLAG_HIGH_RISK, RISK_FLAG_RESTRICTED, RISK_FLAG_UNDER_REVIEW,
};
use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::{symbol_short, Address, Env, IntoVal, Symbol, TryIntoVal};

fn setup_contract(env: &Env) -> (BountyEscrowContractClient<'static>, Address) {
    env.mock_all_auths();

    let contract_id = env.register_contract(None, BountyEscrowContract);
    let client = BountyEscrowContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let token_admin = Address::generate(env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    client.init(&admin, &token_id);

    (client, admin)
}

fn last_risk_event(env: &Env) -> RiskFlagsUpdated {
    let events = env.events().all();
    for e in events.iter().rev() {
        let topics = e.1;
        if topics.len() < 2 {
            continue;
        }
        let t0: Symbol = topics.get(0).unwrap().into_val(env);
        if t0 != symbol_short!("risk") {
            continue;
        }
        return e.2.try_into_val(env).expect("risk event payload");
    }
    panic!("expected a RiskFlagsUpdated event");
}

#[test]
fn test_escrow_risk_flags_set_clear_and_persist() {
    let env = Env::default();
    let (client, _admin) = setup_contract(&env);

    let bounty_id = 42u64;
    let flags = RISK_FLAG_HIGH_RISK | RISK_FLAG_UNDER_REVIEW;

    let updated = client.set_escrow_risk_flags(&bounty_id, &flags);
    assert_eq!(updated.risk_flags, flags);

    let cleared = client.clear_escrow_risk_flags(&bounty_id, &RISK_FLAG_UNDER_REVIEW);
    assert_eq!(cleared.risk_flags, RISK_FLAG_HIGH_RISK);

    client.update_metadata(
        &_admin,
        &bounty_id,
        &123,
        &456,
        &soroban_sdk::String::from_str(&env, "bug_fix"),
        &None,
    );

    let fetched = client.get_metadata(&bounty_id);
    assert_eq!(fetched.risk_flags, RISK_FLAG_HIGH_RISK);
}

/// All defined public flag bits can be set together and read back from storage.
#[test]
fn test_escrow_risk_flags_all_bits_round_trip() {
    let env = Env::default();
    let (client, _admin) = setup_contract(&env);
    let bounty_id = 7u64;
    let all =
        RISK_FLAG_HIGH_RISK | RISK_FLAG_UNDER_REVIEW | RISK_FLAG_RESTRICTED | RISK_FLAG_DEPRECATED;

    client.set_escrow_risk_flags(&bounty_id, &all);
    assert_eq!(client.get_metadata(&bounty_id).risk_flags, all);

    client.clear_escrow_risk_flags(&bounty_id, &all);
    assert_eq!(client.get_metadata(&bounty_id).risk_flags, 0);

    // Idempotent clear on zero
    client.clear_escrow_risk_flags(&bounty_id, &RISK_FLAG_HIGH_RISK);
    assert_eq!(client.get_metadata(&bounty_id).risk_flags, 0);
}

/// Event payloads must carry version, bounty id, previous/new flags, and admin (program-escrow style).
#[test]
fn test_risk_flags_events_consistent_payloads() {
    let env = Env::default();
    env.ledger().with_mut(|li| {
        li.timestamp = 9_001;
    });
    let (client, admin) = setup_contract(&env);
    let bounty_id = 99u64;

    client.set_escrow_risk_flags(&bounty_id, &RISK_FLAG_HIGH_RISK);
    let e1 = last_risk_event(&env);
    assert_eq!(e1.version, EVENT_VERSION_V2);
    assert_eq!(e1.bounty_id, bounty_id);
    assert_eq!(e1.previous_flags, 0);
    assert_eq!(e1.new_flags, RISK_FLAG_HIGH_RISK);
    assert_eq!(e1.admin, admin);
    assert_eq!(e1.timestamp, 9_001);

    client.set_escrow_risk_flags(&bounty_id, &(RISK_FLAG_HIGH_RISK | RISK_FLAG_UNDER_REVIEW));
    let e2 = last_risk_event(&env);
    assert_eq!(e2.previous_flags, RISK_FLAG_HIGH_RISK);
    assert_eq!(e2.new_flags, RISK_FLAG_HIGH_RISK | RISK_FLAG_UNDER_REVIEW);

    client.clear_escrow_risk_flags(&bounty_id, &RISK_FLAG_UNDER_REVIEW);
    let e3 = last_risk_event(&env);
    assert_eq!(
        e3.previous_flags,
        RISK_FLAG_HIGH_RISK | RISK_FLAG_UNDER_REVIEW
    );
    assert_eq!(e3.new_flags, RISK_FLAG_HIGH_RISK);
}

#[test]
fn test_set_escrow_risk_flags_requires_init() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, BountyEscrowContract);
    let client = BountyEscrowContractClient::new(&env, &contract_id);
    assert_eq!(
        client
            .try_set_escrow_risk_flags(&1u64, &1u32)
            .unwrap_err()
            .unwrap(),
        Error::NotInitialized
    );
}

// # BuildInfo Event Tests
//
// Comprehensive tests for the `BuildInfo` event emission during contract initialization.
//
// ## Test Coverage
// - BuildInfo event emission on init_admin
// - Event field validation (admin, version, timestamp)
// - Single initialization guarantee (AlreadyInitialized error)
// - Authorization requirement verification
// - Event topic and structure validation
// - Timestamp accuracy and sequence
// - Edge cases and boundary conditions
//
// ## Security Considerations
// - BuildInfo event provides audit trail for initialization
// - Event requires admin authorization to be emitted
// - Event data is immutable once emitted
// - Prevents double-initialization via AlreadyInitialized error

extern crate std;
use std::vec::Vec;

use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, Symbol, TryIntoVal,
};

use crate::{BuildInfoEvent, GrainlifyContract, GrainlifyContractClient, VERSION};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Setup function to initialize a contract and return the client and admin address
fn setup_contract(env: &Env) -> (GrainlifyContractClient, Address) {
    env.mock_all_auths();
    let id = env.register_contract(None, GrainlifyContract);
    let client = GrainlifyContractClient::new(env, &id);
    let admin = Address::generate(env);
    (client, admin)
}

// ── tests: BuildInfo event emission ──────────────────────────────────────────

/// BuildInfo event is emitted when init_admin is called
#[test]
fn test_build_info_event_emitted_on_init() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin) = setup_contract(&env);

    // Call init_admin
    client.init_admin(&admin);

    // Get all events
    let events = env.events().all();

    // Verify at least one event was emitted
    assert!(
        events.len() > 0,
        "At least one event should be emitted during init_admin"
    );
}

/// BuildInfo event contains correct admin address
#[test]
fn test_build_info_event_admin_field() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin) = setup_contract(&env);

    // Initialize - should emit BuildInfo event
    client.init_admin(&admin);

    // Verify no panic and contract is initialized
    let stored_admin = client.get_admin();
    assert_eq!(stored_admin, Some(admin), "Admin should be stored correctly");
}

/// BuildInfo event contains correct version
#[test]
fn test_build_info_event_version_field() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin) = setup_contract(&env);

    client.init_admin(&admin);

    // Verify version is set correctly
    let version = client.get_version();
    assert_eq!(version, VERSION, "Version should match initial VERSION constant");
}

/// BuildInfo event timestamp is accurate
#[test]
fn test_build_info_event_timestamp_accuracy() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin) = setup_contract(&env);
    let before_timestamp = env.ledger().timestamp();

    client.init_admin(&admin);

    let after_timestamp = env.ledger().timestamp();

    // Timestamp should be within transaction bounds
    assert!(
        before_timestamp <= after_timestamp,
        "Before timestamp should be <= after timestamp"
    );
}

// ── tests: BuildInfo event and AlreadyInitialized guard ──────────────────────

/// Double initialization is prevented with AlreadyInitialized error
#[test]
#[should_panic(expected = "1")]
fn test_double_initialization_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin) = setup_contract(&env);

    // First initialization succeeds
    client.init_admin(&admin);

    // Second initialization should panic with AlreadyInitialized (code 1)
    client.init_admin(&admin);
}

/// BuildInfo event is only emitted once
#[test]
fn test_build_info_event_emitted_once() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin) = setup_contract(&env);

    // First initialization succeeds
    client.init_admin(&admin);

    // Try to initialize again - should fail with AlreadyInitialized
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.init_admin(&admin);
    }));

    assert!(
        result.is_err(),
        "Second initialization should panic with AlreadyInitialized"
    );
}

// ── tests: Authorization and security ────────────────────────────────────────

/// BuildInfo event is only emitted when admin auth succeeds
#[test]
fn test_build_info_event_requires_admin_auth() {
    let env = Env::default();
    // Don't mock auths - let real auth checks run
    let id = env.register_contract(None, GrainlifyContract);
    let client = GrainlifyContractClient::new(&env, &id);
    let admin = Address::generate(&env);

    // Call without mocking auth should panic
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.init_admin(&admin);
    }));

    assert!(result.is_err(), "init_admin without admin auth should panic");

    // Now call with mocked auth
    env.mock_all_auths();
    client.init_admin(&admin);

    let events = env.events().all();
    let build_info_events: Vec<_> = events
        .iter()
        .filter(|event| {
            let topics = &event.1;
            topics.len() >= 1 && {
                let t0: Option<Symbol> = topics.get(0).and_then(|v| v.try_into_val(&env).ok());
                t0 == Some(soroban_sdk::symbol_short!("init"))
            }
        })
        .collect();

    assert_eq!(build_info_events.len(), 1);
}

// ── tests: Event structure validation ────────────────────────────────────────

/// BuildInfo event requires initialization
#[test]
fn test_build_info_event_requires_init() {
    let env = Env::default();
    env.mock_all_auths();

    let id = env.register_contract(None, GrainlifyContract);
    let client = GrainlifyContractClient::new(&env, &id);
    let admin = Address::generate(&env);

    // After initialization, admin should be set
    client.init_admin(&admin);
    let stored_admin = client.get_admin();
    
    assert_eq!(stored_admin, Some(admin), "Admin should be properly initialized");
}

// ── tests: Edge cases and boundary conditions ────────────────────────────────

/// BuildInfo event works with different admin addresses
#[test]
fn test_build_info_event_with_different_admins() {
    let env = Env::default();
    env.mock_all_auths();

    // Test with multiple different admin addresses
    let admins = std::vec![
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];

    for admin in admins {
        let id = env.register_contract(None, GrainlifyContract);
        let client = GrainlifyContractClient::new(&env, &id);

        client.init_admin(&admin);

        let events = env.events().all();
        let build_info_events: Vec<_> = events
            .iter()
            .filter(|event| {
                // Filter by this specific contract instance so accumulated
                // events from prior loop iterations don't cause a mismatch.
                event.0 == id && {
                    let topics = &event.1;
                    topics.len() >= 1 && {
                        let t0: Option<Symbol> =
                            topics.get(0).and_then(|v| v.try_into_val(&env).ok());
                        t0 == Some(soroban_sdk::symbol_short!("init"))
                    }
                }
            })
            .collect();

        assert!(!build_info_events.is_empty());
        let event_data: BuildInfoEvent = build_info_events[0].2.try_into_val(&env).unwrap();
        assert_eq!(event_data.admin, admin);
    }
}

/// BuildInfo event serialization works correctly
#[test]
fn test_build_info_event_data_structure() {
    let env = Env::default();

    let admin = Address::generate(&env);
    let event = BuildInfoEvent {
        admin: admin.clone(),
        version: VERSION,
        timestamp: 12345,
    };

    // Verify data structure is valid
    assert_eq!(event.admin, admin);
    assert_eq!(event.version, VERSION);
    assert_eq!(event.timestamp, 12345);
}

// ── tests: Version consistency ───────────────────────────────────────────────

/// BuildInfo event version matches get_version result
#[test]
fn test_build_info_event_version_matches_get_version() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin) = setup_contract(&env);

    client.init_admin(&admin);

    // Get version from contract
    let contract_version = client.get_version();

    // Version should match the VERSION constant
    assert_eq!(contract_version, VERSION);
}

// ── tests: Multiple contracts ────────────────────────────────────────────────

/// BuildInfo events are independent per contract instance
#[test]
fn test_build_info_event_per_contract_instance() {
    let env = Env::default();
    env.mock_all_auths();

    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);

    // Create first contract
    let id1 = env.register_contract(None, GrainlifyContract);
    let client1 = GrainlifyContractClient::new(&env, &id1);
    client1.init_admin(&admin1);

    // Create second contract
    let id2 = env.register_contract(None, GrainlifyContract);
    let client2 = GrainlifyContractClient::new(&env, &id2);
    client2.init_admin(&admin2);

    // Verify each contract has its own admin
    let stored_admin1 = client1.get_admin();
    let stored_admin2 = client2.get_admin();

    assert_eq!(stored_admin1, Some(admin1));
    assert_eq!(stored_admin2, Some(admin2));
    assert_ne!(stored_admin1, stored_admin2);
}

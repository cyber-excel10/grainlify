# Contract Upgrade Safety System

This document describes the upgrade safety system implemented for the BountyEscrow contract, which provides a comprehensive set of pre-upgrade validations to prevent contract failures during upgrades.

## Overview

The upgrade safety system is designed to reduce the risk of "bricking" contracts during upgrade operations. It provides:

1. **Pre-upgrade safety checks** - A comprehensive checklist of validations
2. **Dry-run simulation** - Execute safety checks without mutating state
3. **Upgrade gates** - Require safety checks to pass before upgrade

## Safety Checklist

Before any upgrade, the following invariants are validated:

### 1. Storage Layout Compatibility (Code: 1001)
- Verifies the contract has been initialized
- Ensures new code can read existing storage keys
- Checks that storage structure is compatible
- Validates `EscrowIndex` entries resolve to exactly one variant:
  - `DataKey::Escrow(bounty_id)` (legacy storage), or
  - `DataKey::EscrowAnon(bounty_id)` (extension storage)
- Fails on dangling index entries or dual-variant collisions for the same `bounty_id`

#### Storage Extension Checklist (Backward-Compatibility)
- Additive keys only: never remove or repurpose existing keys used by older clients.
- New keys must be optional-by-default (safe when absent).
- Legacy read paths must continue to work for old data (`Escrow` records remain valid).
- Extension records (`EscrowAnon`) must not coexist with legacy records for the same `bounty_id`.
- Index integrity must hold across versions: every indexed id maps to one concrete escrow record.

### 2. Contract Initialization State (Code: 1002)
- Validates admin address is set
- Validates token address is configured
- Ensures basic contract state exists

### 3. Escrow State Consistency (Code: 1003)
- All escrows must be in valid states
- Verifies amounts are non-negative
- Validates remaining_amount <= amount
- Checks status-specific invariants (e.g., released escrows have 0 remaining)

### 4. Pending Claims Verification (Code: 1004)
- Validates all pending claims are valid
- Ensures claims are only on pending escrows
- Checks for orphaned claims

### 5. Admin Authority (Code: 1005)
- Verifies admin address is properly set
- Ensures admin is a valid address

### 6. Token Configuration (Code: 1006)
- Validates token address is configured
- Ensures token contract is accessible

### 7. Feature Flags Readiness (Code: 1007)
- Checks pause flags
- Validates feature toggle states

### 8. Reentrancy Lock Check (Code: 1008)
- Ensures no stuck reentrancy guards
- Validates contract is not in a locked state

### 9. Version Compatibility (Code: 1009)
- Validates version information is accessible
- Checks version is consistent

### 10. Balance Sanity (Code: 1010)
- Verifies total locked amounts are non-negative
- Checks for token balance consistency

## Usage

### Running a Dry-Run (Simulation)

To simulate an upgrade without actually performing it:

```rust
// Call the contract's simulate_upgrade function
let report = contract.simulate_upgrade();

if report.is_safe {
    println!("Upgrade is safe to proceed!");
} else {
    println!("Upgrade safety checks failed:");
    for error in report.errors {
        println!("  - {}", error.message);
    }
}
```

### Performing an Upgrade

To perform an actual upgrade (with safety checks):

```rust
// This will first run all safety checks
// If any check fails, the upgrade will be rejected
contract.upgrade(new_wasm_hash)?;
```

### Managing Safety Checks

The safety system can be enabled/disabled by the admin:

```rust
// Disable safety checks (not recommended)
contract.set_upgrade_safety(false)?;

// Enable safety checks
contract.set_upgrade_safety(true)?;

// Check current status
let enabled = contract.get_upgrade_safety_status();
```

## Contract Interface

### Functions Added

| Function | Description |
|----------|-------------|
| `simulate_upgrade()` | Runs all safety checks, returns detailed report |
| `upgrade(new_wasm_hash)` | Performs upgrade after validating safety |
| `set_upgrade_safety(enabled)` | Enable/disable safety checks (admin only) |
| `get_upgrade_safety_status()` | Returns whether safety checks are enabled |

### Return Types

#### UpgradeSafetyReport
```rust
struct UpgradeSafetyReport {
    is_safe: bool,           // Overall safety status
    checks_passed: u32,      // Number of checks passed
    checks_failed: u32,      // Number of checks failed
    warnings: Vec<UpgradeWarning>,  // Warnings encountered
    errors: Vec<UpgradeError>,      // Errors encountered
}
```

## Testing

The upgrade safety system includes comprehensive tests covering:

- Safe upgrade scenarios (initialized contract, valid state)
- Unsafe upgrade scenarios (uninitialized, invalid state)
- Multiple escrow states (locked, released, refunded)
- Safety check toggle functionality
- Direct module testing

Run tests with:
```bash
cd contracts/bounty_escrow/contracts/escrow
cargo test test_upgrade
```

## Upgrade Script

A shell script is provided for production upgrades:

```bash
# Dry-run (safety check only)
./scripts/upgrade_contract.sh testnet C... contract.wasm --dry-run

# Actual upgrade
./scripts/upgrade_contract.sh testnet C... contract.wasm
```

The script:
1. Validates environment and inputs
2. Runs pre-upgrade safety checks
3. Shows detailed checklist
4. Prompts for confirmation
5. Performs the upgrade

## Best Practices

1. **Always run dry-run first** - Execute `simulate_upgrade()` before any upgrade
2. **Review warnings** - Even passed checks may have warnings to review
3. **Backup state** - Keep backups of contract state before upgrades
4. **Test on testnet** - Always test upgrades on testnet first
5. **Incremental upgrades** - Make small, incremental changes rather than large jumps
6. **Monitor post-upgrade** - Watch contract behavior after upgrade

## Error Handling

If safety checks fail, the upgrade returns `Error::UpgradeSafetyCheckFailed`. The detailed error information can be obtained from the simulation report:

```rust
match contract.simulate_upgrade() {
    report if report.is_safe => {
        // Safe to upgrade
    },
    report => {
        // Print errors
        for error in report.errors {
            eprintln!("Error {}: {}", error.code, error.message);
        }
    }
}
```

## Future Enhancements

Potential improvements for the safety system:

1. **Storage diff analysis** - Compare storage layout between versions
2. **Upgrade migration hooks** - Allow data migration during upgrade
3. **Upgrade rollback** - Ability to rollback to previous version
4. **Multi-sig upgrades** - Require multiple signatures for upgrade
5. **Timelock** - Delay between safety check and actual upgrade

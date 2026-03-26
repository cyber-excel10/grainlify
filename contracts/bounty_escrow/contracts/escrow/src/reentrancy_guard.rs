//! # Reentrancy Guard Module
//!
//! Provides cross-function reentrancy protection for the Bounty Escrow contract.
//!
//! ## Threat Model
//!
//! Every function that performs an **external call** — primarily Stellar token
//! transfers via the SAC `transfer` entry-point — is a potential reentrancy
//! vector.  If a malicious token contract invoked a callback that re-entered
//! `release_funds`, `partial_release`, `refund`, `claim`, or any other
//! state-mutating function *before* the first invocation finished, funds
//! could be drained or state left inconsistent.
//!
//! This module places a **single boolean flag** (`DataKey::ReentrancyGuard`)
//! in instance storage.  Because the key is shared across *all* protected
//! functions, the guard blocks both **same-function** and **cross-function**
//! re-entry.
//!
//! ## Checks–Effects–Interactions (CEI) Alignment
//!
//! The guard complements — but does not replace — CEI ordering.  Every
//! protected function should:
//!
//! 1. `acquire` the guard,
//! 2. perform all **checks** (auth, paused, status),
//! 3. commit all **effects** (state writes),
//! 4. execute **interactions** (token transfers, cross-contract calls),
//! 5. `release` the guard.
//!
//! If a function returns early with `Err(..)` or panics, Soroban atomically
//! rolls back all storage mutations (including the guard flag itself), so the
//! guard can never become permanently stuck.
//!
//! ## Protected Functions
//!
//! The following entry-points **must** be wrapped with `acquire` / `release`:
//!
//! | Function                 | External call          |
//! |--------------------------|------------------------|
//! | `lock_funds`             | token `transfer`       |
//! | `lock_funds_anon`        | token `transfer`       |
//! | `release_funds`          | token `transfer`       |
//! | `partial_release`        | token `transfer`       |
//! | `refund`                 | token `transfer`       |
//! | `refund_resolved`        | token `transfer`       |
//! | `refund_with_capability` | token `transfer`       |
//! | `release_with_capability`| token `transfer`       |
//! | `claim`                  | token `transfer`       |
//! | `batch_lock_funds`       | token `transfer` ×N    |
//! | `batch_release_funds`    | token `transfer` ×N    |
//! | `emergency_withdraw`     | token `transfer`       |
//!
//! ## Usage
//!
//! ```rust,ignore
//! use crate::reentrancy_guard;
//!
//! pub fn sensitive_function(env: Env) {
//!     reentrancy_guard::acquire(&env);   // panics on re-entry
//!     // ... checks, state writes, token transfers ...
//!     reentrancy_guard::release(&env);
//! }
//! ```
//!
//! ## Soroban Rollback Guarantee
//!
//! - On `panic!`, Soroban rolls back **all** state changes for the current
//!   invocation, including the guard flag.  The guard therefore cannot become
//!   permanently stuck after an unexpected failure.
//! - Returning `Err(..)` from a `#[contractimpl]` function has the same
//!   rollback semantics, so early-return error paths after `acquire` are safe.

use super::DataKey;
use soroban_sdk::Env;

/// Acquire the reentrancy guard.
///
/// Sets a boolean flag in instance storage.  If the flag is already set,
/// this function panics — indicating a re-entrant call.
///
/// # Panics
///
/// Panics with `"Reentrancy detected"` if the guard has already been
/// acquired and not yet released within the current execution context.
pub fn acquire(env: &Env) {
    if env.storage().instance().has(&DataKey::ReentrancyGuard) {
        panic!("Reentrancy detected");
    }
    env.storage()
        .instance()
        .set(&DataKey::ReentrancyGuard, &true);
}

/// Release the reentrancy guard.
///
/// Removes the guard flag from instance storage, allowing the next
/// top-level invocation to proceed.
///
/// Must be called on the **success path** of every function that called
/// [`acquire`].  On error/panic paths Soroban's automatic state rollback
/// clears the flag, so explicit release is not required there.
pub fn release(env: &Env) {
    env.storage().instance().remove(&DataKey::ReentrancyGuard);
}

/// Query whether the guard is currently held.
///
/// Exposed only in test builds to allow assertions in integration tests.
#[cfg(test)]
pub fn is_active(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::ReentrancyGuard)
}

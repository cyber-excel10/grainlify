//! Asset helpers shared across Grainlify Soroban contracts.
//!
//! This module keeps token handling aligned with the current Soroban token
//! interface by:
//!
//! - validating asset identifiers as contract strkeys,
//! - constructing the current `token::TokenClient`, and
//! - enforcing exact-balance transfer semantics for escrow-style accounting.
//!
//! `validate_asset_id` intentionally only proves that an identifier is a
//! contract address. It does not attempt to probe whether the contract fully
//! implements `TokenInterface`, because cross-contract interface checks would
//! require invoking untrusted code during validation. Callers that need stronger
//! guarantees should use the exact-transfer helpers, which verify observed
//! balance deltas during the actual token operation.

use soroban_sdk::{contracterror, token, Address, Env};

pub type AssetId = Address;
pub type AssetClient<'a> = token::TokenClient<'a>;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AssetIdError {
    MustBeContractAddress = 200,
    TransferAmountMismatch = 201,
}

/// Normalizes an incoming asset identifier to the canonical `AssetId`.
///
/// Normalization is currently identity after validating that the identifier is
/// a contract address suitable for Soroban token operations.
pub fn normalize_asset_id(env: &Env, raw_asset_id: &Address) -> Result<AssetId, AssetIdError> {
    validate_asset_id(env, raw_asset_id)?;
    Ok(raw_asset_id.clone())
}

/// Validates the canonical asset identifier invariants.
///
/// For token operations, asset ids must be Soroban contract addresses.
pub fn validate_asset_id(env: &Env, asset_id: &AssetId) -> Result<(), AssetIdError> {
    let _ = env;
    let strkey = asset_id.to_string();
    if strkey.len() != 56 {
        return Err(AssetIdError::MustBeContractAddress);
    }

    let mut bytes = [0u8; 56];
    strkey.copy_into_slice(&mut bytes);
    if bytes[0] == b'C' {
        Ok(())
    } else {
        Err(AssetIdError::MustBeContractAddress)
    }
}

/// Returns a Soroban token client for the validated asset identifier.
pub fn token_client<'a>(env: &'a Env, asset_id: &AssetId) -> Result<AssetClient<'a>, AssetIdError> {
    validate_asset_id(env, asset_id)?;
    Ok(token::TokenClient::new(env, asset_id))
}

/// Returns the balance for `holder` from the validated token contract.
pub fn balance(env: &Env, asset_id: &AssetId, holder: &Address) -> Result<i128, AssetIdError> {
    let client = token_client(env, asset_id)?;
    Ok(client.balance(holder))
}

/// Transfers exactly `amount` from `from` to `to`.
///
/// This helper rejects fee-on-transfer or otherwise non-standard token
/// behaviour by verifying that sender and recipient balances changed by the
/// requested amount. Escrow contracts rely on exact accounting, so silently
/// accepting smaller recipient deltas would create under-collateralized state.
pub fn transfer_exact(
    env: &Env,
    asset_id: &AssetId,
    from: &Address,
    to: &Address,
    amount: i128,
) -> Result<(), AssetIdError> {
    if amount == 0 || from == to {
        return Ok(());
    }

    let client = token_client(env, asset_id)?;
    let before_from = client.balance(from);
    let before_to = client.balance(to);

    client.transfer(from, to, &amount);

    let after_from = client.balance(from);
    let after_to = client.balance(to);

    let debited = before_from
        .checked_sub(after_from)
        .ok_or(AssetIdError::TransferAmountMismatch)?;
    let credited = after_to
        .checked_sub(before_to)
        .ok_or(AssetIdError::TransferAmountMismatch)?;

    if debited == amount && credited == amount {
        Ok(())
    } else {
        Err(AssetIdError::TransferAmountMismatch)
    }
}


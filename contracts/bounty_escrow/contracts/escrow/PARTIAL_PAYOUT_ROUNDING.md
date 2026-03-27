# Partial Payout Rounding Rules

This document describes the rounding behavior and invariants for partial payouts in the bounty escrow contract.

## Overview

The bounty escrow contract supports partial releases, allowing a bounty to be paid out incrementally to one or more contributors. This document defines the rounding rules that ensure no tokens are lost or created during partial payouts.

## Integer Arithmetic

All token amounts use `i128` integer arithmetic. There is no floating-point or fixed-point arithmetic involved in partial payouts. This means:

- All subtractions are exact (no rounding errors)
- No dust is created from arithmetic operations
- The smallest transferable unit is 1 (one stroop for XLM-based tokens)

## Core Invariants

### Sum Invariant

At any point during the lifecycle of an escrow:

```
sum(all_payouts) + remaining_amount == original_locked_amount
```

This invariant is enforced by:
1. Using `checked_sub` for all decrements to `remaining_amount`
2. Rejecting any payout that exceeds `remaining_amount`

### Balance Conservation

The contract's token balance always equals the sum of `remaining_amount` across all active (Locked) escrows:

```
token.balance(contract) == sum(escrow.remaining_amount for all Locked escrows)
```

### No Dust Creation

Partial releases never create or destroy tokens:
- Exactly `payout_amount` tokens are transferred to the recipient
- Exactly `payout_amount` is subtracted from `remaining_amount`
- No rounding occurs in either operation

## Dust Handling

"Dust" refers to very small remainder amounts after partial payouts. The contract handles dust correctly:

1. **No minimum payout**: Any amount >= 1 can be released
2. **No minimum remainder**: Any non-zero remainder is valid
3. **Retrievable dust**: Even 1-unit remainders can be released or refunded

## Partial Release Rules

1. **Positive amount required**: `payout_amount` must be > 0
2. **Within bounds**: `payout_amount` must be <= `remaining_amount`
3. **Exact transfer**: The exact requested amount is transferred
4. **Status transition**: When `remaining_amount` reaches 0, status becomes `Released`

## Fee Calculations

Fee calculations (when applicable) use floor division:

```
fee = (amount * fee_basis_points) / 10_000
```

This means:
- Fees are rounded down to the nearest integer
- The remainder stays with the recipient
- Fee rounding is handled separately from partial payout logic

## Test Coverage

The `test_partial_payout_rounding.rs` module provides comprehensive tests:

- Single-unit (dust-level) payouts
- Tiny remainder scenarios
- Multiple sequential micro-payouts
- Large amounts with small payouts
- Sum invariant verification
- Balance conservation checks
- Overpayment prevention
- Cross-bounty isolation

## Security Considerations

1. **Overflow protection**: All arithmetic uses checked operations
2. **Underflow prevention**: Payouts cannot exceed remaining amount
3. **State consistency**: Status transitions only occur at zero remainder
4. **Isolation**: Partial releases on one bounty do not affect others

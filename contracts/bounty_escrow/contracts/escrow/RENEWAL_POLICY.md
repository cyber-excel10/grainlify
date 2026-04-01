# Renewal And Rollover Policy

This policy defines how bounty lifetime extension (`renew_escrow`) and cycle rollover (`create_next_cycle`) behave in `bounty-escrow`.

## Renewal (`renew_escrow`)

`renew_escrow(bounty_id, new_deadline, additional_amount)` updates a currently active escrow without replacing its `bounty_id`.

Rules:
- Escrow must exist and be in `Locked` status.
- Renewal must happen before expiry (`now < current_deadline`).
- `new_deadline` must be strictly greater than the current deadline.
- `additional_amount` may be `0` (deadline-only extension) or positive (top-up).
- Negative `additional_amount` is rejected.
- The original depositor must authorize the renewal.

State effects:
- `deadline` is set to `new_deadline`.
- If `additional_amount > 0`, both `amount` and `remaining_amount` are increased exactly by `additional_amount`.
- Funds are transferred from depositor to contract for top-ups.
- A `RenewalRecord` is appended to immutable renewal history.

## Rollover (`create_next_cycle`)

`create_next_cycle(previous_bounty_id, new_bounty_id, amount, deadline)` starts a new cycle as a fresh escrow while preserving prior-cycle history.

Rules:
- Previous escrow must exist and be finalized (`Released` or `Refunded`).
- Previous cycle may have only one direct successor.
- `new_bounty_id` must not already exist and must differ from `previous_bounty_id`.
- `amount` must be strictly positive.
- `deadline` must be in the future.
- The original depositor authorizes funding for the new cycle.

State effects:
- New `Escrow` is created in `Locked` status.
- New funds are transferred from depositor to contract.
- Cycle links are updated:
  - previous `next_id = new_bounty_id`
  - new `previous_id = previous_bounty_id`
  - new cycle depth increments by one.

## Security Notes

- No post-expiry resurrection: renewal after deadline is rejected.
- No hidden balance loss: renew without top-up does not change token balances.
- No double-successor forks: rollover chain enforces one successor per cycle.
- Renewal history is append-only and remains available after rollover.

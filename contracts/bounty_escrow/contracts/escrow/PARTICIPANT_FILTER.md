# Participant filtering (allowlist / blocklist)

The escrow contract supports **mutually exclusive** participant filtering modes for `lock_funds` and `batch_lock_funds`. Only one mode is active at a time.

## Modes

| Mode            | Value | Behavior |
|-----------------|-------|----------|
| **Disabled**    | `0`   | No list check. Any address may lock funds. The allowlist (whitelist) is still used only for **anti-abuse bypass** (rate limit / cooldown). |
| **BlocklistOnly** | `1` | Blocklisted addresses cannot lock; all others can. |
| **AllowlistOnly** | `2` | Only allowlisted (whitelisted) addresses can lock; all others get `ParticipantNotAllowed`. |

Default after init is **Disabled**.

## Admin API

- **`set_filter_mode(mode)`** — Set the active mode (admin only). Emits `ParticipantFilterModeChanged` (topic `pf_mode`) with `previous_mode`, `new_mode`, `admin`, `timestamp`.
- **`get_filter_mode()`** — View current mode.
- **`set_whitelist_entry(address, true|false)`** — Add/remove from allowlist (admin only). In AllowlistOnly mode this controls who can participate; in other modes it only affects anti-abuse bypass.
- **`set_blocklist_entry(address, true|false)`** — Add/remove from blocklist (admin only). Only enforced when mode is BlocklistOnly.

## Errors

- **`ParticipantBlocked`** (35) — Mode is BlocklistOnly and the depositor is blocklisted.
- **`ParticipantNotAllowed`** (36) — Mode is AllowlistOnly and the depositor is not on the allowlist.

## State transitions

- Switching mode does **not** clear list data. Allowlist and blocklist entries persist across mode changes.
- **Corner cases:**
  - If you switch from AllowlistOnly to Disabled, anyone can lock until you switch back or set BlocklistOnly.
  - If you switch to AllowlistOnly without adding any addresses, **no one** can lock until you add allowlist entries.
  - An address can be both allowlisted and blocklisted; the **current mode** decides which list is used. In BlocklistOnly, blocklist wins (blocked); in AllowlistOnly, allowlist wins (must be allowlisted).
- Mode changes are evented so indexers and UIs can reflect the current policy.

## Where it applies

Participant filtering is enforced only at **lock** time:

- `lock_funds(depositor, ...)` — `depositor` is checked against the current mode.
- `batch_lock_funds(items)` — each `item.depositor` is checked; if any fails, the whole batch reverts.
- Anti-abuse cooldown and rate-limit checks still run after the participant filter check unless the depositor is allowlisted.

Release, refund, and other operations do **not** check participant mode; only who can **create** new escrows is restricted.

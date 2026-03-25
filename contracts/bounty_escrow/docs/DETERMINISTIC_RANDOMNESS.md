# Deterministic Randomness — Bounty Escrow Claim-Ticket Selection

## Overview

The bounty escrow contract uses **deterministic pseudo-randomness** to select a
winner from a candidate list when issuing claim tickets.  The algorithm is
implemented in `grainlify_core::pseudo_randomness::derive_selection` and exposed
through three contract entry-points:

| Entry-point                          | Purpose                                   |
|--------------------------------------|-------------------------------------------|
| `derive_claim_ticket_winner_index`   | View helper — returns the winning index   |
| `derive_claim_ticket_winner`         | View helper — returns the winning address |
| `issue_claim_ticket_deterministic`   | Mutating — selects winner and issues ticket |

## How It Works

### Context Construction

`build_claim_selection_context` assembles a byte buffer from:

1. **Contract address** — binds to a specific deployment.
2. **Bounty ID** — isolates each bounty's selection space.
3. **Claim amount** — different payouts yield different outcomes.
4. **Ticket expiry** (`expires_at`) — time-bound parameter.
5. **Ledger timestamp** (`env.ledger().timestamp()`) — ties the selection to the
   block that executes the transaction.
6. **Ticket counter** — monotonically increasing on-chain counter; prevents
   identical results across successive calls within the same ledger close.

### Seed Hashing

The context bytes, a domain tag (`claim_prng_v1`), and a caller-provided
32-byte `external_seed` are concatenated and hashed with SHA-256 to produce a
`seed_hash`.

### Per-Candidate Scoring

For each candidate address, `SHA-256(seed_hash ‖ candidate_xdr)` produces a
32-byte score.  The candidate with the **lexicographically highest** score wins.
Because every candidate is scored independently, the result is
**order-independent** — shuffling the candidate list does not change the winner.

## Predictability Limits

> **This is not true randomness.**

The selection is fully deterministic given the inputs listed above.  Anyone who
can reconstruct those inputs can predict the outcome.

### Threat Model

| Adversary              | Capability                                  | Risk |
|------------------------|---------------------------------------------|------|
| **External observer**  | Knows published event fields after the fact | Can verify but not influence — low risk |
| **Seed grinder**       | Tries many `external_seed` values off-chain | Can find a seed that selects a preferred candidate — **medium risk** |
| **Timing manipulator** | Submits transaction only at favorable timestamps | Can bias outcome by choosing when to submit — **medium risk** |
| **Validator**          | Knows ledger timestamp before block close   | Can predict outcome for any given seed — **high risk** |
| **Candidate stuffer**  | Adds sybil addresses to the candidate list  | Increases probability of controlling the winner — **medium risk** |

### Why Deterministic?

Soroban does not currently provide a native source of on-chain randomness (like
`RANDAO` on Ethereum).  A deterministic approach was chosen because:

- Results are **auditable and replayable** from published event data.
- No additional oracle infrastructure is required.
- The selection is transparent — any party can independently verify correctness.

### Mitigations

To reduce the impact of the predictability limits above, integrators should:

1. **Use commit–reveal schemes** for the external seed: the caller commits a
   hash of their seed in one transaction and reveals it in a later one,
   preventing seed grinding after observing ledger state.
2. **Publish seed sources** (e.g., block hash of a prior ledger) so that
   observers can verify the seed was not cherry-picked.
3. **Limit candidate registration windows** to prevent last-minute sybil
   stuffing.
4. **Monitor `DeterministicSelectionDerived` events** for anomalous patterns
   (e.g., the same caller always winning across multiple bounties).

## Event Auditability

Every call to `issue_claim_ticket_deterministic` emits a
`DeterministicSelectionDerived` event containing:

| Field                  | Description                                  |
|------------------------|----------------------------------------------|
| `bounty_id`            | The bounty that triggered the selection      |
| `selected_index`       | Index of the winning candidate               |
| `candidate_count`      | Total number of candidates                   |
| `selected_beneficiary` | Address of the winning candidate             |
| `seed_hash`            | SHA-256 of (domain ‖ context ‖ external_seed)|
| `winner_score`         | SHA-256 score of the winning candidate       |
| `timestamp`            | Ledger timestamp at selection time           |

Off-chain verifiers can recompute `seed_hash` and per-candidate scores from
the published inputs to confirm the selection was executed correctly.

## Test Coverage

The test suite in `test_deterministic_randomness.rs` verifies:

- **Stability** — identical inputs always produce the same winner.
- **Ledger binding** — advancing the ledger timestamp changes the outcome.
- **Seed sensitivity** — different external seeds yield different winners.
- **Bounty-ID sensitivity** — different bounty IDs change the selection.
- **Amount sensitivity** — different claim amounts change the selection.
- **Expiry sensitivity** — different expiry timestamps change the selection.
- **Order independence** — shuffling candidates does not change the winner.
- **Single candidate** — a sole candidate is always selected.
- **Index ↔ address agreement** — `derive_claim_ticket_winner_index` and
  `derive_claim_ticket_winner` return consistent results.
- **Ticket monotonicity** — successive deterministic tickets get increasing IDs.
- **Integration** — `issue_claim_ticket_deterministic` succeeds end-to-end with
  locked funds.

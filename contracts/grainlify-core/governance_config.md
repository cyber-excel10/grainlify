# Governance Configuration Guide

This document outlines the configuration parameters and security model for the Grainlify Governance system.

## Configuration Parameters

The `GovernanceConfig` struct defines how the governance system operates. These parameters are set during initialization and are critical for the security and efficiency of the protocol.

| Parameter | Type | Description |
|-----------|------|-------------|
| `voting_period` | `u64` | The duration (in seconds) that a proposal remains open for voting. |
| `execution_delay` | `u64` | The time (in seconds) that must pass after a proposal is approved before it can be executed. This allows users to exit the protocol if they disagree with a passed proposal. |
| `quorum_percentage` | `u32` | The minimum percentage of total voting power required to participate for a vote to be valid. Expressed in basis points (100 = 1%). |
| `approval_threshold` | `u32` | The minimum percentage of "For" votes relative to total decisive votes ("For" + "Against") required for a proposal to pass. Expressed in basis points. |
| `min_proposal_stake` | `i128` | The amount of governance tokens a proposer must stake to create a proposal. This prevents spam. |
| `voting_scheme` | `VotingScheme` | Determines how voting power is calculated: `OnePersonOneVote` or `TokenWeighted`. |
| `governance_token` | `Address` | The address of the Soroban token used for staking and weighted voting. |

## Proposal Lifecycle

1. **Active**: A proposal is created and is immediately open for voting.
2. **Finalization**: Once the `voting_period` has passed, anyone can call `finalize_proposal`.
    - If quorum and approval thresholds are met, the status becomes `Approved`.
    - Otherwise, it becomes `Rejected`.
3. **Approved**: The proposal is waiting for the `execution_delay` to pass.
4. **Executed**: After the delay, `execute_proposal` can be called to apply the WASM upgrade.

## Security Assumptions

- **Admin Control**: Only the designated administrator can initialize the governance system.
- **Staking**: The `min_proposal_stake` ensures that proposers have "skin in the game" and discourages malicious or low-quality proposals.
- **Quorum**: Prevents a small minority from passing controversial changes.
- **Execution Delay**: A critical security feature that provides a window for users to react to approved changes before they are applied.

## Error Enums

The system uses specific error codes to provide clear feedback on failures:

- `NotInitialized`: The system must be initialized before use.
- `InvalidThreshold`: Quorum or approval thresholds must be between 0 and 10000 bps.
- `InsufficientStake`: Proposer lacks the required tokens to stake.
- `AlreadyVoted`: Double voting is strictly prohibited.
- `ExecutionDelayNotMet`: Proposals cannot be executed before the mandatory delay.

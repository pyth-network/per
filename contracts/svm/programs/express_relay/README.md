# Express Relay Program

This subdir contains the Express Relay program and its SDK to help integrating programs check permissioning and perform testing.

## Express Relay Design

The design of Express Relay on Solana utilizes the fact that Solana allows for multiple instructions to different programs within the same transaction. As a result, one of the instructions in the Express Relay instruction will be the `SubmitBid` instruction of the Express Relay program, using a `permission` account that represents the specific position/vault whose permissioning is being auctioned by Express Relay. This `permission` pubkey could represent the address of a user's position (or the keccak hash of relevant identifying information for a particular position). This `SubmitBid` instruction will also handle validating and distributing to the relevant parties (protocol, relayer, Express Relay admin) the bid.

The Express Relay `SubmitBid` instruction must be signed by a keypair belonging to the relayer, which is set by governance. It should also be signed by the wallet of the searcher from which the bid will be extracted in SOL (this will often also be the keypair that signs the integrating program instruction(s)). The `SubmitBid` instruction will also contain an account `protocol` representing the integrating program/app. This can be the address of an executable account (most likely the address of an integrating program), in which case the `fee_receiver_protocol` should be a specified PDA of this program. It could alternatively be the pubkey of a keypair (e.g. for frontends looking to integrate), in which case `fee_recevier_protocol` should be the same as `protocol`. The `protocol_config` account specifies the protocol-specific config PDA that, if it exists, has protocol-specific fee splits.

The integrating program will need to check that the appropriate `SubmitBid` instruction is one of the instructions in the ongoing transaction. It can do this by calling the `CheckPermission` method of the Express Relay program via CPI. Note that the permissioning is not stored in any state; it is simply retrieved from the instructions of the current transaction.

To integrate with Express Relay, the integrating program needs to make the following changes:

1. Store the pubkey of the express relay program or validate it in the program logic
2. Add a `permission` account to the relevant instruction being gated
3. Make a CPI to the Express Relay program `CheckPermission` instruction
4. If planning to receive fees in a PDA of the program, create the relevant PDA specified by the seeds in the Express Relay `SubmitBid` instruction.

## Example Integration

Integrating programs can use the `check_permission` helper method defined in the Express Relay SDK:

```rust
use anchor_lang::prelude::*;
use express_relay::sdk::cpi::check_permission;

#[program]
pub mod integrating_program {
    use super::*;

    pub fn do_something(ctx: Context<DoSomething>, data: DoSomethingArgs) -> Result<()> {
        check_permission(
            ctx.accounts.sysvar_instructions.to_account_info(),
            ctx.accounts.permission.to_account_info(),
            ctx.accounts.router.to_account_info(),
        )

        /// integrating_program do_something logic
    }
}
```

To run Rust-based tests, an integrating program can use the helper methods defined in `src/sdk/test_helpers.rs`. See the [dummy example](https://github.com/pyth-network/per/tree/main/contracts/svm/programs/dummy) to see how these methods can be used for Rust-based end-to-end testing.

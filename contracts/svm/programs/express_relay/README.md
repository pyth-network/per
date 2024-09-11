# Express Relay Program

This subdir contains the Express Relay program and its SDK to help integrating programs check permissioning and perform testing.

## Express Relay Design

The design of Express Relay on Solana utilizes the fact that Solana allows for multiple instructions to different programs within the same transaction. As a result, one of the instructions in the Express Relay transaction will be the `SubmitBid` instruction of the Express Relay program, using a `permission` account that represents the specific opportunity (e.g. trade; vault liquidation) whose permissioning is being auctioned by Express Relay. For example, this `permission` pubkey could be the address of a user's position (or the keccak hash of relevant identifying information for a particular position). The `SubmitBid` instruction will also handle validating and distributing to the relevant parties (router, relayer) the bid.

The Express Relay `SubmitBid` instruction has two signers: a keypair belonging to the relayer, which is set by governance, and the wallet of the searcher from which the bid will be extracted in SOL (this will often also be the keypair that signs the integrating program instruction(s)). Before submitting the transaction with the `SubmitBid` instruction, a searcher must sign that transaction with all the necessary keypairs except for the relayer keypair. The `SubmitBid` instruction will also contain an account `router` representing the integrating program/app. This could be a PDA of the integrating program, the address of a relevant user (e.g. limit order maker) in the transaction workflow, or an address controlled by a protocol's DAO or an app's owner--whoever the integrating program/app wants to receive the fees from Express Relay. The `config_router` account specifies the router-specific config PDA that, if it exists, has router-specific fee splits (which would have been set by governance).

The integrating program will need to check that the appropriate `SubmitBid` instruction is one of the instructions in the ongoing transaction. It can do this by calling the `CheckPermission` method of the Express Relay program via CPI. Note that the permissioning is not stored in any state; it is simply retrieved from the instructions of the current transaction.

To integrate with Express Relay, the integrating program needs to make the following changes:

1. Store the pubkey of the express relay program and validate it in the program logic
2. Add a `permission` account to the relevant instruction being gated, or use a suitable existing account for the permissioning check.
3. Make a CPI to the Express Relay program `CheckPermission` instruction with the designated `permission` and `router` accounts.
4. If planning to receive fees in a PDA of the program, create the relevant PDA specified by the seeds in the Express Relay `SubmitBid` instruction beforehand.

## Example Integration

Integrating programs can use the `check_permission_cpi` helper method defined in the Express Relay SDK:

```rust
use anchor_lang::prelude::*;
use express_relay::sdk::cpi::check_permission;

#[program]
pub mod integrating_program {
    use super::*;

    pub fn do_something(ctx: Context<DoSomething>, data: DoSomethingArgs) -> Result<()> {
        let check_permission_accounts = CheckPermission {
            sysvar_instructions:    ctx.accounts.sysvar_instructions.to_account_info(),
            permission:             ctx.accounts.permission.to_account_info(),
            router:                 ctx.accounts.router.to_account_info(),
            config_router:          ctx.accounts.config_router.to_account_info(),
            express_relay_metadata: ctx.accounts.express_relay_metadata.to_account_info(),
        };
        let (n_bid_ixs, fees) = check_permission_cpi(
            check_permission_accounts,
            ctx.accounts.express_relay.to_account_info(),
        )?;

        /// integrating_program do_something logic
    }
}
```

Some integrating programs may need to learn how much will be paid in fees in an ongoing transaction. The `check_permission_cpi` returns a tuple with the number of bid instructions matching the specified `permission` and `router` and the fees paid to the router in the current transaction. An example use of this can be seen in the [dummy example program](https://github.com/pyth-network/per/tree/main/contracts/svm/programs/dummy).

To run Rust-based tests, an integrating program can use the helper methods defined in `src/sdk/test_helpers.rs`. See the [dummy example](https://github.com/pyth-network/per/tree/main/contracts/svm/programs/dummy) to see how these methods can be used for Rust-based end-to-end testing.

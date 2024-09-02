# Express Relay (Solana)

This subdir contains:

- Express Relay program
- Test cases
- Helper methods and programs (e.g. dummy program) for tests

## Express Relay Design

The design of Express Relay on Solana utilizes the fact that Solana allows for multiple instructions to different programs within the same transaction. As a result, one of the instructions in the Express Relay instruction will be the `SubmitBid` instruction of the Express Relay program, using a `permission` account that represents the specific position/vault whose permissioning is being auctioned by Express Relay. This `permission` pubkey could represent the address of a user's position (or the keccak hash of relevant identifying information for a particular position). This `SubmitBid` instruction will also handle validating and distributing to the relevant parties (protocol, relayer, Express Relay admin) the bid.

The Express Relay `SubmitBid` instruction must be signed by a keypair belonging to the relayer, which is set by governance. It should also be signed by the wallet of the searcher from which the bid will be extracted in SOL (this will often also be the keypair that signs the integrating program instruction(s)). The `SubmitBid` instruction will also contain an account `protocol` representing the integrating program/app. This can be the address of an executable account (most likely the address of an integrating program), in which case the `fee_receiver_protocol` should be a specified PDA of this program. It could alternatively be the pubkey of a keypair (e.g. for frontends looking to integrate), in which case `fee_recevier_protocol` should be the same as `protocol`. The `protocol_config` account specifies the protocol-specific config PDA that, if it exists, has protocol-specific fee splits.

The integrating program will need to check that the appropriate `SubmitBid` instruction is one of the instructions in the ongoing transaction. It can do this by calling the `CheckPermission` method of the Express Relay program via CPI. Note that the permissioning is not stored in any state; it is simply retrieved from the instructions of the current transaction.

To integrate with Express Relay, the integrating program needs to make the following changes:

1. Store the pubkey of the express relay program or validate it in the program logic
2. Add a `permission` account to the relevant instruction being gated
3. Make a CPI to the Express Relay program `CheckPermission` instruction
4. If planning to receive fees in a PDA of the program, create the relevant PDA specified by the seeds in the Express Relay `SubmitBid` instruction.

### Off-chain Flow

Searchers will construct `Transaction`s and submit these to the Express Relay server. These transactions will be validated by the relayer to ensure that they include an Express Relay `SubmitBid` instruction and are successful in simulation. From this the relayer will also ascertain the bids of the different submissions. They will then select a `Transaction` to forward on-chain based on the bids and sign it and submit it.

### Dummy example

The provided `dummy` example program has a `DoNothing` instruction that simply checks the provided permissioning. It does this via CPI to the Express Relay `CheckPermission` instruction. The tests in `testing/tests/` include some integration tests for Express Relay permissioning using this simple program.

## Building and running

Build with `cargo build-sbf`, and run tests with `cargo test`.

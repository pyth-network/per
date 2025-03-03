# Express Relay (Solana)

This subdir contains:

- Express Relay program
- Test cases
- Helper methods and programs (e.g. dummy program) for tests

## [Express Relay Program](contracts/svm/programs/express_relay/README.md)

## Off-chain Flow

Searchers will construct `Transaction`s and submit these to the Express Relay server. These transactions will be validated by the relayer to ensure that they include an Express Relay `SubmitBid` instruction and are successful in simulation. From this the relayer will also ascertain the bids of the different submissions. They will then select a `Transaction` to forward on-chain based on the bids and sign it and submit it.

## Dummy example

The provided `dummy` example program has a `DoNothing` instruction that simply checks the provided permissioning. It does this via CPI to the Express Relay `CheckPermission` instruction via a helper method in the Express Relay SDK. The tests in `programs/dummy/tests` showcase how an integrating program can use the Express Relay SDK to perform end-to-end testing with Express Relay.

## Express Relay Tests

The tests in `testing/tests/` include some integration tests for Express Relay permissioning
using the simple `dummy` program as the integrating program.
To run these tests, run `cargo-build-sbf` followed by `cargo test-sbf -p dummy` from `contracts/svm`.
As of now, using `cargo test` will not succeed due to [existing issues in the Anchor version of the `processor!` macro](https://github.com/coral-xyz/anchor/pull/2711). Alternatively you can run all tests with `SBF_OUT_DIR="../../target/deploy" cargo test`.

## Building and running

Build with `cargo build-sbf`, and run tests with `cargo test -p testing`.

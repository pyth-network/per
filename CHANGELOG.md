# Changelog

All notable changes to the searcher sdks will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Rust: 0.13.0, Python 0.28.0, Javascript 0.29.0, api-types 0.12.0] - 2025-05-14

- Failed bids for swap quotes now have a reason status that exposes why the transaction failed on-chain. [554](https://github.com/pyth-network/per/pull/554)

## [Rust: 0.12.0, Python 0.27.0, Javascript 0.28.0, api-types 0.11.0] - 2025-05-09

- Add a profile ID field to the opportunity api type. [538](https://github.com/pyth-network/per/pull/538)

## [Rust: 0.11.0, Python 0.26.0, Javascript 0.27.0] - 2025-05-07

- Create swap instruction now uses Swap v2 of the Express Relay contract. [530](https://github.com/pyth-network/per/pull/530)

## [Rust: 0.10.0, Python 0.25.0, Javascript 0.26.0] - 2025-04-24

- Support for EVM chains has been removed. The SDKs no longer support EVM-compatible chains. [495](https://github.com/pyth-network/per/pull/495) [513](https://github.com/pyth-network/per/pull/513) [516](https://github.com/pyth-network/per/pull/516)

## [Rust: 0.9.0, Python 0.24.0, Javascript 0.25.0] - 2025-04-15

### Added

- The minimum lifetime of swap quotes is now configurable. Users can provide a `minimum_lifetime` which represents for how long the quote they receive should be valid and defaults to the old value of 10 seconds. Searchers receive a `minimum_deadline` as a timestamp in the opportunity parameters that they should use as the deadline for their quotes. [482](https://github.com/pyth-network/per/pull/482)
- Now the user can request non-cancellable quotes: these are quotes where the searcher can't call `cancel_bid` while the server is waiting for the user signature. These opportunities have the flag `cancellable` set to false in the opportunity parameters. [481](https://github.com/pyth-network/per/pull/481)
- A new status `submission_failed` has been added to bids. This status is used when a user tries to submit a bid on-chain, but the submission fails. It includes a `reason` field to explain why the submission did not succeed. The possible reasons are:
  - `cancelled`: The bid was cancelled by the owner before the user could submit it on-chain.
  - `deadline_passed`: The user attempted to submit the bid too late, after the bid deadline had already passed. [489](https://github.com/pyth-network/per/pull/489)

## [Rust: 0.8.0, Python 0.23.0, Javascript 0.24.0] - 2025-04-08

### Added

- For swap opportunities, the searcher sdks will now add a memo instruction to the bid transaction if the quote requester so desires. This allows the quote requester to track which on-chain transactions correspond to quotes they requested. [458](https://github.com/pyth-network/per/pull/458)

### Fixed

- Fixed a bug in the Python SDK where it expected the variant for swap opportunities to be `phantom` instead of `swap`. [443](https://github.com/pyth-network/per/pull/443)
- For swap opportunities, when a user wants to swap SOL but doesn't have enough funds, the sdk will never try to wrap (on behalf of the user) an amount exceeding the SOL balance of the user. [461](https://github.com/pyth-network/per/pull/461)
- Made the Python searcher SDK forward compatible with adding new bid statuses. [469](https://github.com/pyth-network/per/pull/469)

## [Rust: 0.7.0, Python 0.22.0, Javascript 0.23.0] - 2025-03-25

### Changed

- For swap opportunities, the searcher sdks now only add `Create Associated Token Account Idempotent` instructions for user token accounts and fee token accounts when these accounts don't yet exist (instead of always). [428](https://github.com/pyth-network/per/pull/428)
- For swap opportunities, users are now responsible to pay for their own associated token accounts in the swap transactions (to receive the searcher token and to trasact with Wrapped SOL) unless they have very low SOL balance. In the previous version, searcher paid for all token account creations. [428](https://github.com/pyth-network/per/pull/428)
- For swap opportunities, the searchers sdks now close the user's Wrapped SOL account after the user sends Wrapped SOL to the searcher. This improves the user's UX by returning the rent of their Wrapped SOL ATA. [434](https://github.com/pyth-network/per/pull/434)

### Fixed

- Fixed a bug where the amount of user SOL needed to be wrapped was underestimated. This bug affected swap opportunities where the amount of searcher token was specified, fee was paid in the user token and the user token was Wrapped SOL. [432](https://github.com/pyth-network/per/pull/432)

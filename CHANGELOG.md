# Changelog

All notable changes to the searcher sdks will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- For swap opportunities, the searcher sdks will now add a memo instruction to the bid transaction if the quote requester so desires. This allows the quote requester to track which on-chain transactions correspond to quotes they requested. [458](https://github.com/pyth-network/per/pull/458)

### Fixed

- For swap opportunities, when a user wants to swap SOL but doesn't have enough funds, the sdk will never try to wrap (on behalf of the user) an amount exceeding the SOL balance of the user.

## [Rust: 0.7.0, Python 0.22.0, Javascript 0.23.0] - 2025-03-25

### Changed

- For swap opportunities, the searcher sdks now only add `Create Associated Token Account Idempotent` instructions for user token accounts and fee token accounts when these accounts don't yet exist (instead of always). [428](https://github.com/pyth-network/per/pull/428)
- For swap opportunities, users are now responsible to pay for their own associated token accounts in the swap transactions (to receive the searcher token and to trasact with Wrapped SOL) unless they have very low SOL balance. In the previous version, searcher paid for all token account creations. [428](https://github.com/pyth-network/per/pull/428)
- For swap opportunities, the searchers sdks now close the user's Wrapped SOL account after the user sends Wrapped SOL to the searcher. This improves the user's UX by returning the rent of their Wrapped SOL ATA. [434](https://github.com/pyth-network/per/pull/434)

### Fixed

- Fixed a bug where the amount of user SOL needed to be wrapped was underestimated. This bug affected swap opportunities where the amount of searcher token was specified, fee was paid in the user token and the user token was Wrapped SOL. [432](https://github.com/pyth-network/per/pull/432)

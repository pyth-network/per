// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

// Signature: 0xd5668c88
error NotCalledByExpressRelay();

// Signature: 0xb02436cc
error NotCalledByFactory();

// Signature: 0x446f3eeb
error AdapterOwnerMismatch();

// Signature: 0x4af147aa
error InsufficientTokenReceived();

// Signature: 0x9caaa1d7
error InsufficientEthToSettleBid();

// Signature: 0x5e520cd4
error InsufficientWethForTargetCallValue();

// Signature: 0xa932c97a
error TargetCallFailed(bytes returnData);

// Signature: 0x464e3f6a
error DuplicateToken();

// Signature: 0x1979776d
error EthOrWethBalanceDecreased();

// Signature: 0x879bab83
error TargetContractNotAllowed(uint256 index);

// Signature: 0x47a8ea58
error OnlyOwnerCanCall();

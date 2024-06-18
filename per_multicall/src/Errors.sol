// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

// Signature: 0x82b42900
error Unauthorized();

// Signature: 0x868a64de
error InvalidPermission();

// Signature: 0x8baa579f
error InvalidSignature();

// Signature: 0xdf4cc36d
error ExpiredSignature();

// Signature: 0x900bb2c9
error SignatureAlreadyUsed();

// Signature: 0x464e3f6a
error DuplicateToken();

// Signature: 0x1979776d
error EthOrWethBalanceDecreased();

// Signature: 0x729f3230
error InvalidPERSignature();

// Signature: 0xb7d09497
error InvalidTimestamp();

// Signature: 0xaba47339
error NotRegistered();

// Signature: 0xa932c97a
error TargetCallFailed(bytes returnData);

// Signature: 0x4af147aa
error InsufficientTokenReceived();

// Signature: 0x9caaa1d7
error InsufficientEthToSettleBid();

// Signature: 0x5e520cd4
error InsufficientWethForTargetCallValue();

// Signature: 0x4be6321b
error InvalidSignatureLength();
// The new contract does not have the same magic value as the old one.
// Signature: 0x4ed848c1
error InvalidMagicValue();

// Signature: 0x0601f697
error InvalidFeeSplit();

// Signature: 0xb40d37c3
error DuplicateRelayerSubwallet();

// Signature: 0xac4d92b3
error RelayerSubwalletNotFound();

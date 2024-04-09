// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
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

// Signature: 0xd461d1de
error WethTransferFromFailed();

// Signature: 0x729f3230
error InvalidPERSignature();

// Signature: 0xb7d09497
error InvalidTimestamp();

// Signature: 0xaba47339
error NotRegistered();

// Signature: 0x714ed4ea
error TargetCallFailed(string reason);

// Signature: 0x4af147aa
error InsufficientTokenReceived();

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

// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import "../CommonErrors.sol";

// Signature: 0x868a64de
error InvalidPermission();

//// The new contract does not have the same magic value as the old one.
//// Signature: 0x4ed848c1
//error InvalidMagicValue();

// Signature: 0x0601f697
error InvalidFeeSplit();

// Signature: 0xb40d37c3
error DuplicateRelayerSubwallet();

// Signature: 0xac4d92b3
error RelayerSubwalletNotFound();

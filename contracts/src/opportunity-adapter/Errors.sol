// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import "../CommonErrors.sol";

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

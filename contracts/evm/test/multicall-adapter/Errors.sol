// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

// Signature: 0xa3022410
error MulticallAdapterInsufficientWethForTargetCallValue();

// Signature: 0xf5fd1b90
error MulticallAdapterTargetCallFailed(
    uint256 targetCallIndex,
    bytes returnData
);

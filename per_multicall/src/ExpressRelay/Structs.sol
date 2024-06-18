// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

struct MulticallData {
    bytes16 bidId;
    address targetContract;
    bytes targetCalldata;
    uint256 bidAmount;
}

struct MulticallStatus {
    bool externalSuccess;
    bytes externalResult;
    string multicallRevertReason;
}

// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import "permit2/interfaces/ISignatureTransfer.sol";

struct OracleState {
    uint256 price;
    uint256 timestamp;
}

struct Vault {
    address tokenCollateral;
    address tokenDebt;
    uint256 amountCollateral;
    uint256 amountDebt;
    uint256 minHealthRatio; // 10**18 is 100%
    uint256 minPermissionlessHealthRatio;
    bytes32 tokenIdCollateral;
    bytes32 tokenIdDebt;
}

struct TokenAmount {
    address token;
    uint256 amount;
}

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

struct ExecutionWitness {
    TokenAmount[] buyTokens;
    address executor;
    address targetContract;
    bytes targetCalldata;
    uint256 targetCallValue;
    uint256 bidAmount;
}

struct ExecutionParams {
    ISignatureTransfer.PermitBatchTransferFrom permit;
    ExecutionWitness witness;
}

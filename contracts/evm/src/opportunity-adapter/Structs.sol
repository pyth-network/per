// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import "permit2/interfaces/ISignatureTransfer.sol";

struct TokenAmount {
    address token;
    uint256 amount;
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

// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "permit2/interfaces/ISignatureTransfer.sol";

struct TokenAmount {
    address token;
    uint256 amount;
}

struct ExecutionWitness {
    TokenAmount[] buyTokens;
    address owner;
}

struct ExecutionParams {
    ISignatureTransfer.PermitBatchTransferFrom permit;
    ExecutionWitness witness;
}

// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "permit2/interfaces/ISignatureTransfer.sol";

struct TokenAmount {
    address token;
    uint256 amount;
}

struct OpportunityProviderExecutionWitness {
    TokenAmount[] buyTokens;
    address owner;
    bytes permissionKey;
}

struct OpportunityProviderExecutionParams {
    ISignatureTransfer.PermitBatchTransferFrom permit;
    OpportunityProviderExecutionWitness witness;
}

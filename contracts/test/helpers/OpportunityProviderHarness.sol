// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {OpportunityProvider} from "../opportunity-provider/OpportunityProvider.sol";
import {TokenAmount, ExecutionWitness} from "../opportunity-provider/Structs.sol";
import "permit2/interfaces/ISignatureTransfer.sol";

contract OpportunityProviderHarness is OpportunityProvider {
    constructor(
        address admin,
        address expressRelay,
        address permit2
    ) OpportunityProvider(admin, expressRelay, permit2) {}

    function exposed_transferSellTokens(
        ISignatureTransfer.PermitBatchTransferFrom calldata permit,
        ExecutionWitness calldata witness,
        bytes calldata signature
    ) public {
        _transferSellTokens(permit, witness, signature);
    }

    function exposed_checkDuplicateTokensTokenAmount(
        TokenAmount[] calldata tokens
    ) public pure {
        _checkDuplicateTokens(tokens);
    }

    function exposed_checkDuplicateTokensTokenPermissions(
        ISignatureTransfer.TokenPermissions[] calldata tokens
    ) public pure {
        _checkDuplicateTokens(tokens);
    }

    function exposed_transferBuyTokens(
        ExecutionWitness calldata witness
    ) public {
        _transferBuyTokens(witness);
    }
}

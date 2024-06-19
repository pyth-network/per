// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {OpportunityProvider} from "../../src/OpportunityProvider.sol";
import {TokenAmount, OpportunityProviderExecutionWitness} from "../../src/Structs.sol";
import "permit2/interfaces/ISignatureTransfer.sol";

contract OpportunityProviderHarness is OpportunityProvider {
    function exposed_transferSellTokens(
        ISignatureTransfer.PermitBatchTransferFrom calldata permit,
        OpportunityProviderExecutionWitness calldata witness,
        bytes calldata signature
    ) public {
        _transferSellTokens(permit, witness, signature);
    }

    function exposed_checkDuplicateTokensTokenAmount(
        TokenAmount[] calldata tokens
    ) public {
        _checkDuplicateTokens(tokens);
    }

    function exposed_checkDuplicateTokensTokenPermissions(
        ISignatureTransfer.TokenPermissions[] calldata tokens
    ) public {
        _checkDuplicateTokens(tokens);
    }

    function exposed_transferBuyTokens(
        OpportunityProviderExecutionWitness calldata witness
    ) public {
        _transferBuyTokens(witness);
    }
}

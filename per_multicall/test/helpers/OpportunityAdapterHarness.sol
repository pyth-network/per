// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {OpportunityAdapter} from "../../src/OpportunityAdapter.sol";
import {TokenAmount, ExecutionWitness, PermitBatchTransferFrom} from "../../src/Structs.sol";

contract OpportunityAdapterHarness is OpportunityAdapter {
    constructor(address permit2) {
        _initialize(msg.sender, address(0), address(0), permit2);
    }

    function exposed_prepareSellTokens(
        PermitBatchTransferFrom calldata permit,
        ExecutionWitness calldata witness,
        bytes calldata signature
    ) public {
        _prepareSellTokens(permit, witness, signature);
    }

    function exposed_revokeAllowances(
        PermitBatchTransferFrom calldata permit,
        address targetContract
    ) public {
        _revokeAllowances(permit, targetContract);
    }

    function exposed_checkDuplicateTokens(
        TokenAmount[] calldata tokens
    ) public {
        _checkDuplicateTokens(tokens);
    }

    function exposed_getContractTokenBalances(
        TokenAmount[] calldata tokens
    ) public returns (uint256[] memory) {
        return _getContractTokenBalances(tokens);
    }

    function exposed_validateAndTransferBuyTokens(
        TokenAmount[] calldata buyTokens,
        address executor,
        uint256[] memory buyTokensBalancesBeforeCall
    ) public {
        _validateAndTransferBuyTokens(
            buyTokens,
            executor,
            buyTokensBalancesBeforeCall
        );
    }
}

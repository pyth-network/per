// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {OpportunityAdapter} from "../../src/OpportunityAdapter.sol";
import {TokenAmount} from "../../src/Structs.sol";

contract OpportunityAdapterHarness is OpportunityAdapter {
    function exposed_PrepareSellTokens(
        TokenAmount[] calldata sellTokens,
        address executor,
        address targetContract
    ) public {
        _prepareSellTokens(sellTokens, executor, targetContract);
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

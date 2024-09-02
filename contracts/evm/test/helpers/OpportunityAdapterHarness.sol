// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import {OpportunityAdapter, TokenAmount, ExecutionWitness} from "src/opportunity-adapter/OpportunityAdapter.sol";
import "permit2/interfaces/ISignatureTransfer.sol";

contract OpportunityAdapterHarness is OpportunityAdapter {
    function exposed_prepareSellTokens(
        ISignatureTransfer.PermitBatchTransferFrom calldata permit,
        ExecutionWitness calldata witness,
        bytes calldata signature
    ) public {
        _prepareSellTokens(permit, witness, signature);
    }

    function getOpportunityWitnessType() public view returns (string memory) {
        return _OPPORTUNITY_WITNESS_TYPE;
    }

    function getTokenAmountType() public view returns (string memory) {
        return _TOKEN_AMOUNT_TYPE;
    }

    function exposed_revokeAllowances(
        ISignatureTransfer.PermitBatchTransferFrom calldata permit,
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

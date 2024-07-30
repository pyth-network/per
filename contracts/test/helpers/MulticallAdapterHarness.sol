// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import {MulticallAdapter, TokenAmount, TokenToSend, TargetCall, MulticallParams} from "test/multicall-adapter/MulticallAdapter.sol";
import "permit2/interfaces/ISignatureTransfer.sol";

contract MulticallAdapterHarness is MulticallAdapter {
    function exposed_transferSellTokens(TokenAmount[] calldata tokens) public {
        _transferSellTokens(tokens);
    }

    function exposed_approveTokens(TokenToSend[] calldata tokensToSend) public {
        _approveTokens(tokensToSend);
    }

    function exposed_callTargetContract(
        address targetContract,
        bytes calldata targetCalldata,
        uint256 targetCallValue
    ) public {
        _callTargetContract(targetContract, targetCalldata, targetCallValue);
    }

    function exposed_revokeAllowances(
        TokenToSend[] calldata tokensToSend
    ) public {
        _revokeAllowances(tokensToSend);
    }

    function exposed_sweepTokensTokenAmount(
        TokenAmount[] calldata tokens
    ) public {
        _sweepTokens(tokens);
    }

    function exposed_sweepTokensTokenToSend(
        TokenToSend[] calldata tokensToSend
    ) public {
        _sweepTokens(tokensToSend);
    }
}

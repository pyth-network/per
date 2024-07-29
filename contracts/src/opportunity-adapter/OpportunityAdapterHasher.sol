// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import "./Structs.sol";

contract OpportunityAdapterHasher {
    string internal constant _OPPORTUNITY_WITNESS_TYPE =
        "OpportunityWitness(TokenAmount[] buyTokens,address executor,TargetCall[] targetCalls,uint256 bidAmount)TargetCall(address targetContract,bytes targetCalldata,uint256 targetCallValue,TokenToSend[] tokensToSend)TokenToSend(TokenAmount tokenAmount,address destination)TokenAmount(address token,uint256 amount)";

    string internal constant _TOKEN_AMOUNT_TYPE =
        "TokenAmount(address token,uint256 amount)";

    string internal constant _TARGET_CALL_TYPE =
        "TargetCall(address targetContract,bytes targetCalldata,uint256 targetCallValue,TokenToSend[] tokensToSend)TokenToSend(TokenAmount tokenAmount,address destination)TokenAmount(address token,uint256 amount)";

    string internal constant _TOKEN_TO_SEND_TYPE =
        "TokenToSend(TokenAmount tokenAmount,address destination)TokenAmount(address token,uint256 amount)";

    function hash(
        TokenAmount memory tokenAmount
    ) internal pure returns (bytes32) {
        return
            keccak256(
                abi.encode(
                    keccak256(bytes(_TOKEN_AMOUNT_TYPE)),
                    tokenAmount.token,
                    tokenAmount.amount
                )
            );
    }

    function hash(
        TokenAmount[] memory tokenAmounts
    ) internal pure returns (bytes32) {
        bytes32[] memory hashedTokens = new bytes32[](tokenAmounts.length);
        for (uint i = 0; i < tokenAmounts.length; i++) {
            hashedTokens[i] = hash(tokenAmounts[i]);
        }
        return keccak256(abi.encodePacked(hashedTokens));
    }

    function hash(
        TargetCall memory targetCall
    ) internal pure returns (bytes32) {
        return
            keccak256(
                abi.encode(
                    keccak256(bytes(_TARGET_CALL_TYPE)),
                    targetCall.targetContract,
                    keccak256(targetCall.targetCalldata),
                    targetCall.targetCallValue,
                    hash(targetCall.tokensToSend)
                )
            );
    }

    function hash(
        TargetCall[] memory targetCalls
    ) internal pure returns (bytes32) {
        bytes32[] memory hashedTargetCalls = new bytes32[](targetCalls.length);
        for (uint i = 0; i < targetCalls.length; i++) {
            hashedTargetCalls[i] = hash(targetCalls[i]);
        }
        return keccak256(abi.encodePacked(hashedTargetCalls));
    }

    function hash(
        TokenToSend memory tokenToSend
    ) internal pure returns (bytes32) {
        return
            keccak256(
                abi.encode(
                    keccak256(bytes(_TOKEN_TO_SEND_TYPE)),
                    hash(tokenToSend.tokenAmount),
                    tokenToSend.destination
                )
            );
    }

    function hash(
        TokenToSend[] memory tokensToSend
    ) internal pure returns (bytes32) {
        bytes32[] memory hashedTokensToSend = new bytes32[](
            tokensToSend.length
        );
        for (uint i = 0; i < tokensToSend.length; i++) {
            hashedTokensToSend[i] = hash(tokensToSend[i]);
        }
        return keccak256(abi.encodePacked(hashedTokensToSend));
    }

    function hash(
        ExecutionWitness memory params
    ) public pure returns (bytes32) {
        return
            keccak256(
                abi.encode(
                    keccak256(bytes(_OPPORTUNITY_WITNESS_TYPE)),
                    hash(params.buyTokens),
                    params.executor,
                    hash(params.targetCalls),
                    params.bidAmount
                )
            );
    }
}

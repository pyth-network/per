// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import "./Structs.sol";

contract OpportunityAdapterHasher {
    string internal constant _OPPORTUNITY_WITNESS_TYPE =
        "OpportunityWitness(TokenAmount[] buyTokens,bytes targetCalldata,uint256 targetCallValue,address targetContract,address executor,uint256 bidAmount)TokenAmount(address token,uint256 amount)";
    string internal constant _TOKEN_AMOUNT_TYPE =
        "TokenAmount(address token,uint256 amount)";

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
        ExecutionWitness memory params
    ) public pure returns (bytes32) {
        return
            keccak256(
                abi.encode(
                    keccak256(bytes(_OPPORTUNITY_WITNESS_TYPE)),
                    hash(params.buyTokens),
                    keccak256(params.targetCalldata),
                    params.targetCallValue,
                    params.targetContract,
                    params.executor,
                    params.bidAmount
                )
            );
    }
}

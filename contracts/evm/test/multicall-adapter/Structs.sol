// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;
import "permit2/interfaces/ISignatureTransfer.sol";

struct TokenAmount {
    address token;
    uint256 amount;
}

struct TokenToSend {
    TokenAmount tokenAmount;
    address destination;
}

struct TargetCall {
    address targetContract;
    bytes targetCalldata;
    uint256 targetCallValue;
    TokenToSend[] tokensToSend;
}

struct MulticallParams {
    TokenAmount[] sellTokens;
    TokenAmount[] buyTokens;
    TargetCall[] targetCalls;
}

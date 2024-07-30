// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import "./Structs.sol";
import "./Errors.sol";
import "src/opportunity-adapter/IWETH9.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "openzeppelin-contracts/contracts/utils/ReentrancyGuard.sol";

import "forge-std/console.sol";

contract MulticallAdapter is ReentrancyGuard {
    using SafeERC20 for IERC20;

    function _transferSellTokens(TokenAmount[] calldata tokens) internal {
        for (uint i = 0; i < tokens.length; i++) {
            uint256 amount = tokens[i].amount;
            IERC20 token = IERC20(tokens[i].token);
            token.safeTransferFrom(msg.sender, address(this), amount);
        }
    }

    function _makeTargetCall(TargetCall calldata targetCall) internal {
        _approveTokens(targetCall.tokensToSend);
        _callTargetContract(
            targetCall.targetContract,
            targetCall.targetCalldata,
            targetCall.targetCallValue
        );
        _revokeAllowances(targetCall.tokensToSend);
    }

    function _approveTokens(TokenToSend[] calldata tokensToSend) internal {
        for (uint i = 0; i < tokensToSend.length; i++) {
            IERC20 token = IERC20(tokensToSend[i].tokenAmount.token);
            uint256 amount = tokensToSend[i].tokenAmount.amount;
            token.forceApprove(tokensToSend[i].destination, amount);
        }
    }

    function _callTargetContract(
        address targetContract,
        bytes calldata targetCalldata,
        uint256 targetCallValue
    ) internal {
        (bool success, bytes memory returnData) = targetContract.call{
            value: targetCallValue
        }(targetCalldata);
        if (!success) {
            revert MulticallAdapterTargetCallFailed(returnData);
        }
    }

    function _revokeAllowances(TokenToSend[] calldata tokensToSend) internal {
        for (uint i = 0; i < tokensToSend.length; i++) {
            IERC20 token = IERC20(tokensToSend[i].tokenAmount.token);
            token.forceApprove(tokensToSend[i].destination, 0);
        }
    }

    function _sweepTokens(TokenAmount[] calldata tokens) internal {
        for (uint i = 0; i < tokens.length; i++) {
            IERC20 token = IERC20(tokens[i].token);
            uint256 tokenBalance = token.balanceOf(address(this));
            if (tokenBalance > 0) {
                token.safeTransfer(msg.sender, tokenBalance);
            }
        }
    }

    function _sweepTokens(TokenToSend[] calldata tokens) internal {
        for (uint i = 0; i < tokens.length; i++) {
            TokenAmount memory tokenAmount = tokens[i].tokenAmount;
            IERC20 token = IERC20(tokenAmount.token);
            uint256 tokenBalance = token.balanceOf(address(this));
            if (tokenBalance > 0) {
                token.safeTransfer(msg.sender, tokenBalance);
            }
        }
    }

    function multicall(
        MulticallParams calldata params
    ) public payable nonReentrant {
        _transferSellTokens(params.sellTokens);

        for (uint i = 0; i < params.targetCalls.length; i++) {
            _makeTargetCall(params.targetCalls[i]);
        }

        _sweepTokens(params.buyTokens);
        _sweepTokens(params.sellTokens);
        for (uint i = 0; i < params.targetCalls.length; i++) {
            _sweepTokens(params.targetCalls[i].tokensToSend);
        }
    }
}

// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import "./Structs.sol";
import "./IWETH9.sol";
import "./Errors.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";
import "openzeppelin-contracts/contracts/utils/ReentrancyGuard.sol";
import "permit2/interfaces/ISignatureTransfer.sol";
import "./IOpportunityAdapterFactory.sol";
import {OpportunityAdapterHasher} from "./OpportunityAdapterHasher.sol";

contract OpportunityAdapter is ReentrancyGuard, OpportunityAdapterHasher {
    using SafeERC20 for IERC20;

    address immutable _opportunityAdapterFactory;
    address immutable _owner;
    address immutable _expressRelay;
    address immutable _weth;
    address immutable _permit2;

    string public constant WITNESS_TYPE_STRING =
        "OpportunityWitness witness)OpportunityWitness(TokenAmount[] buyTokens,address executor,TargetCall[] targetCalls,uint256 bidAmount)TargetCall(address targetContract,bytes targetCalldata,uint256 targetCallValue,TokenToSend[] tokensToSend)TokenToSend(TokenAmount tokenAmount,address destination)TokenAmount(address token,uint256 amount)TokenPermissions(address token,uint256 amount)";

    /**
     * @notice OpportunityAdapter initializer - Initializes a new opportunity adapter contract
     *
     */
    constructor() {
        _opportunityAdapterFactory = msg.sender;
        (_expressRelay, _weth, _permit2, _owner) = IOpportunityAdapterFactory(
            msg.sender
        ).parameters();
    }

    modifier onlyOwner() {
        if (msg.sender != _owner) {
            revert OnlyOwnerCanCall();
        }
        _;
    }

    /**
     * @notice getOwner function - returns the address of the owner of the contract
     */
    function getOwner() public view returns (address) {
        return _owner;
    }

    /**
     * @notice getExpressRelay function - returns the address of the express relay authenticated for calling this contract
     */
    function getExpressRelay() public view returns (address) {
        return _expressRelay;
    }

    /**
     * @notice getWeth function - returns the address of the WETH contract used for wrapping and unwrapping ETH
     */
    function getWeth() public view returns (address) {
        return _weth;
    }

    /**
     * @notice withdrawEth function - withdraws ETH from the contract to the owner
     */
    function withdrawEth() public onlyOwner {
        (bool sent, ) = payable(_owner).call{value: address(this).balance}("");
        require(sent, "Withdrawal of ETH failed");
    }

    /**
     * @notice withdrawToken function - withdraws specified tokens from the contract to the owner
     *
     * @param token: address of the token to withdraw
     */
    function withdrawToken(address token) public onlyOwner {
        IERC20(token).safeTransfer(
            _owner,
            IERC20(token).balanceOf(address(this))
        );
    }

    function _getWethContract() internal view returns (IWETH9) {
        return IWETH9(payable(_weth));
    }

    function _verifyParams(ExecutionParams calldata params) internal view {
        if (params.witness.executor != _owner) {
            revert AdapterOwnerMismatch();
        }
        for (uint i = 0; i < params.witness.targetCalls.length; i++) {
            if (
                params.witness.targetCalls[i].targetContract == _permit2 ||
                params.witness.targetCalls[i].targetContract == address(this)
            ) {
                revert TargetContractNotAllowed(i);
            }
        }
        _checkDuplicateTokens(params.permit.permitted);
        _checkDuplicateTokens(params.witness.buyTokens);
    }

    function _checkDuplicateTokens(
        TokenAmount[] calldata tokens
    ) internal pure {
        for (uint i = 0; i < tokens.length; i++) {
            for (uint j = i + 1; j < tokens.length; j++) {
                if (tokens[i].token == tokens[j].token) {
                    revert DuplicateToken();
                }
            }
        }
    }

    function _checkDuplicateTokens(
        ISignatureTransfer.TokenPermissions[] calldata tokens
    ) internal pure {
        for (uint i = 0; i < tokens.length; i++) {
            for (uint j = i + 1; j < tokens.length; j++) {
                if (tokens[i].token == tokens[j].token) {
                    revert DuplicateToken();
                }
            }
        }
    }

    function _prepareSellTokens(
        ISignatureTransfer.PermitBatchTransferFrom calldata permit,
        ExecutionWitness calldata witness,
        bytes calldata signature
    ) internal {
        ISignatureTransfer.SignatureTransferDetails[]
            memory transferDetails = new ISignatureTransfer.SignatureTransferDetails[](
                permit.permitted.length
            );
        for (uint i = 0; i < permit.permitted.length; i++) {
            uint256 amount = permit.permitted[i].amount;
            IERC20 token = IERC20(permit.permitted[i].token);
            transferDetails[i] = ISignatureTransfer.SignatureTransferDetails({
                to: address(this),
                requestedAmount: amount
            });
        }
        ISignatureTransfer(_permit2).permitWitnessTransferFrom(
            permit,
            transferDetails,
            witness.executor,
            hash(witness),
            WITNESS_TYPE_STRING,
            signature
        );
    }

    function _revokeAllowances(TokenToSend[] calldata tokensToSend) internal {
        for (uint i = 0; i < tokensToSend.length; i++) {
            IERC20 token = IERC20(tokensToSend[i].tokenAmount.token);
            token.forceApprove(tokensToSend[i].destination, 0);
        }
    }

    function _settleBid(uint256 bidAmount) internal {
        if (bidAmount == 0) return;
        IWETH9 weth = _getWethContract();
        uint256 balance = address(this).balance;
        if (balance < bidAmount) {
            // withdraw from WETH if necessary to pay the bid
            try weth.withdraw(bidAmount - balance) {} catch {
                revert InsufficientEthToSettleBid();
            }
        }
        (bool sent, ) = getExpressRelay().call{value: bidAmount}("");
        require(sent, "Bid transfer to express relay failed");
    }

    function _makeTargetCall(TargetCall calldata targetCall) internal {
        if (targetCall.targetCallValue > 0) {
            IWETH9 weth = _getWethContract();
            try weth.withdraw(targetCall.targetCallValue) {} catch {
                revert InsufficientWethForTargetCallValue();
            }
        }
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
            revert TargetCallFailed(returnData);
        }
    }

    function _getContractTokenBalances(
        TokenAmount[] calldata tokens
    ) internal view returns (uint256[] memory) {
        uint256[] memory tokenBalances = new uint256[](tokens.length);
        for (uint i = 0; i < tokens.length; i++) {
            IERC20 token = IERC20(tokens[i].token);
            tokenBalances[i] = token.balanceOf(address(this));
        }
        return tokenBalances;
    }

    function _validateAndTransferBuyTokens(
        TokenAmount[] calldata buyTokens,
        address executor,
        uint256[] memory buyTokensBalancesBeforeCall
    ) internal {
        for (uint i = 0; i < buyTokens.length; i++) {
            IERC20 token = IERC20(buyTokens[i].token);
            uint256 tokenBalance = token.balanceOf(address(this));
            if (
                tokenBalance <
                buyTokensBalancesBeforeCall[i] + buyTokens[i].amount
            ) {
                revert InsufficientTokenReceived();
            }
            token.safeTransfer(executor, tokenBalance);
        }
    }

    function _getEthAndWethBalances() internal view returns (uint256, uint256) {
        return (
            address(this).balance,
            _getWethContract().balanceOf(address(this))
        );
    }

    function executeOpportunity(
        ExecutionParams calldata params,
        bytes calldata signature
    ) public payable nonReentrant {
        if (msg.sender != _opportunityAdapterFactory) {
            revert NotCalledByFactory();
        }
        _verifyParams(params);
        (
            uint256 ethBalanceBeforeCall,
            uint256 wethBalanceBeforeCall
        ) = _getEthAndWethBalances();
        // get balances of buy tokens before transferring sell tokens since there might be overlaps
        uint256[]
            memory buyTokensBalancesBeforeCall = _getContractTokenBalances(
                params.witness.buyTokens
            );
        _prepareSellTokens(params.permit, params.witness, signature);
        for (uint i = 0; i < params.witness.targetCalls.length; i++) {
            _makeTargetCall(params.witness.targetCalls[i]);
        }
        _settleBid(params.witness.bidAmount);
        _validateAndTransferBuyTokens(
            params.witness.buyTokens,
            params.witness.executor,
            buyTokensBalancesBeforeCall
        );
        (
            uint256 ethBalanceAfterCall,
            uint256 wethBalanceAfterCall
        ) = _getEthAndWethBalances();
        if (
            ethBalanceAfterCall < ethBalanceBeforeCall ||
            wethBalanceAfterCall < wethBalanceBeforeCall
        ) {
            revert EthOrWethBalanceDecreased();
        }
    }

    function version() public pure returns (string memory) {
        return "0.2.0";
    }

    // necessary to receive ETH from WETH contract using withdraw
    receive() external payable {}
}

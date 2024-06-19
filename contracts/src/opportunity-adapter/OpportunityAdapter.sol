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

    address _owner;
    address _expressRelay;
    address _weth;
    address _permit2;

    string public constant WITNESS_TYPE_STRING =
        "OpportunityWitness witness)OpportunityWitness(TokenAmount[] buyTokens,address executor,address targetContract,bytes targetCalldata,uint256 targetCallValue,uint256 bidAmount)TokenAmount(address token,uint256 amount)TokenPermissions(address token,uint256 amount)";

    /**
     * @notice OpportunityAdapter initializer - Initializes a new opportunity adapter contract
     *
     */
    constructor() {
        (_expressRelay, _weth, _permit2, _owner) = IOpportunityAdapterFactory(
            msg.sender
        ).parameters();
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

    function _getWethContract() internal view returns (IWETH9) {
        return IWETH9(payable(_weth));
    }

    function _verifyParams(ExecutionParams calldata params) internal view {
        if (params.witness.executor != _owner) {
            revert AdapterOwnerMismatch();
        }
        if (
            params.witness.targetContract == _permit2 ||
            params.witness.targetContract == address(this)
        ) {
            revert TargetContractNotAllowed();
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
            token.forceApprove(witness.targetContract, amount);
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

    function _revokeAllowances(
        ISignatureTransfer.PermitBatchTransferFrom calldata permit,
        address targetContract
    ) internal {
        for (uint i = 0; i < permit.permitted.length; i++) {
            IERC20 token = IERC20(permit.permitted[i].token);
            token.forceApprove(targetContract, 0);
        }
    }

    function _settleBid(address executor, uint256 bidAmount) internal {
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
            if (
                token.balanceOf(address(this)) <
                buyTokensBalancesBeforeCall[i] + buyTokens[i].amount
            ) {
                revert InsufficientTokenReceived();
            }
            token.safeTransfer(executor, buyTokens[i].amount);
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
        if (params.witness.targetCallValue > 0) {
            IWETH9 weth = _getWethContract();
            try weth.withdraw(params.witness.targetCallValue) {} catch {
                revert InsufficientWethForTargetCallValue();
            }
        }
        _callTargetContract(
            params.witness.targetContract,
            params.witness.targetCalldata,
            params.witness.targetCallValue
        );
        _revokeAllowances(params.permit, params.witness.targetContract);
        _validateAndTransferBuyTokens(
            params.witness.buyTokens,
            params.witness.executor,
            buyTokensBalancesBeforeCall
        );
        _settleBid(params.witness.executor, params.witness.bidAmount);
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
        return "0.1.0";
    }

    // necessary to receive ETH from WETH contract using withdraw
    receive() external payable {}
}

// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Structs.sol";
import "./ExpressRelay.sol";
import "./WETH9.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";
import "permit2/interfaces/ISignatureTransfer.sol";

abstract contract OpportunityAdapter {
    using SafeERC20 for IERC20;

    address _admin;
    address _expressRelay;
    address _weth;
    string public constant _OPPORTUNITY_WITNESS_TYPE =
        "OpportunityWitness(TokenAmount[] buyTokens,address executor,address targetContract,bytes targetCalldata,uint256 targetCallValue,uint256 bidAmount)TokenAmount(address token,uint256 amount)";
    string public constant _TOKEN_AMOUNT_TYPE =
        "TokenAmount(address token,uint256 amount)";

    string public constant WITNESS_TYPE_STRING =
        "OpportunityWitness witness)OpportunityWitness(TokenAmount[] buyTokens,address executor,address targetContract,bytes targetCalldata,uint256 targetCallValue,uint256 bidAmount)TokenAmount(address token,uint256 amount)TokenPermissions(address token,uint256 amount)";

    ISignatureTransfer constant PERMIT2 =
        ISignatureTransfer(0x000000000022D473030F116dDEE9F6B43aC78BA3);

    /**
     * @notice OpportunityAdapter initializer - Initializes a new opportunity adapter contract with given parameters
     *
     * @param admin: address of admin of opportunity adapter
     * @param expressRelay: address of express relay
     * @param weth: address of WETH contract
     */
    function _initialize(
        address admin,
        address expressRelay,
        address weth
    ) internal {
        _admin = admin;
        _expressRelay = expressRelay;
        _weth = weth;
    }

    /**
     * @notice setExpressRelay function - sets the address of the express relay authenticated for calling this contract
     *
     * @param expressRelay: address of express relay contract
     */
    function setExpressRelay(address expressRelay) public {
        if (msg.sender != _admin) {
            revert Unauthorized();
        }
        _expressRelay = expressRelay;
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

    function _getWethContract() internal view returns (WETH9) {
        return WETH9(payable(_weth));
    }

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
                    params.executor,
                    params.targetContract,
                    keccak256(params.targetCalldata),
                    params.targetCallValue,
                    params.bidAmount
                )
            );
    }

    function _verifyParams(
        ExecutionParams calldata params,
        bytes memory signature
    ) internal view {
        if (msg.sender != _expressRelay) {
            revert Unauthorized();
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
            token.approve(witness.targetContract, amount);
        }
        PERMIT2.permitWitnessTransferFrom(
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
            token.approve(targetContract, 0);
        }
    }

    function _settleBid(address executor, uint256 bidAmount) internal {
        if (bidAmount == 0) return;
        WETH9 weth = _getWethContract();
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
    ) public payable {
        _verifyParams(params, signature);
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
            WETH9 weth = _getWethContract();
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

    // necessary to receive ETH from WETH contract using withdraw
    receive() external payable {}
}

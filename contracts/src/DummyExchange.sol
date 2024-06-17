// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Structs.sol";
import "./Errors.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";
import "permit2/interfaces/ISignatureTransfer.sol";
import "openzeppelin-contracts/contracts/utils/ReentrancyGuard.sol";

abstract contract DummyExchange is ReentrancyGuard {
    using SafeERC20 for IERC20;

    ISignatureTransfer _permit2;

    string public constant _DUMMY_EXCHANGE_WITNESS_TYPE =
        "ExchangeWitness(TokenAmount[] buyTokens,address owner)TokenAmount(address token,uint256 amount)";
    string public constant _TOKEN_AMOUNT_TYPE =
        "TokenAmount(address token,uint256 amount)";

    string public constant DUMMY_WITNESS_TYPE_STRING =
        "ExchangeWitness witness)ExchangeWitness(TokenAmount[] buyTokens,address owner)TokenAmount(address token,uint256 amount)TokenPermissions(address token,uint256 amount)";

    /**
     * @notice DummyExchange initializer - Initializes a new dummy exchange contract with given parameters
     *
     * @param permit2: address of permit2 of dummy exchange
     */
    function _initialize(address permit2) internal {
        _permit2 = ISignatureTransfer(permit2);
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
        DummyExchangeExecutionWitness memory params
    ) public pure returns (bytes32) {
        return
            keccak256(
                abi.encode(
                    keccak256(bytes(_DUMMY_EXCHANGE_WITNESS_TYPE)),
                    hash(params.buyTokens),
                    params.owner
                )
            );
    }

    function _verifyParams(
        DummyExchangeExecutionParams calldata params
    ) internal pure {
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

    function _transferSellTokens(
        ISignatureTransfer.PermitBatchTransferFrom calldata permit,
        DummyExchangeExecutionWitness calldata witness,
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
                to: msg.sender,
                requestedAmount: amount
            });
        }
        _permit2.permitWitnessTransferFrom(
            permit,
            transferDetails,
            witness.owner,
            hash(witness),
            DUMMY_WITNESS_TYPE_STRING,
            signature
        );
    }

    function _transferBuyTokens(
        DummyExchangeExecutionWitness calldata witness
    ) internal {
        for (uint i = 0; i < witness.buyTokens.length; i++) {
            IERC20 token = IERC20(witness.buyTokens[i].token);
            token.safeTransferFrom(
                msg.sender,
                witness.owner,
                witness.buyTokens[i].amount
            );
        }
    }

    function executeExchange(
        DummyExchangeExecutionParams calldata params,
        bytes calldata signature
    ) public nonReentrant {
        _verifyParams(params);
        _transferSellTokens(params.permit, params.witness, signature);
        _transferBuyTokens(params.witness);
    }
}

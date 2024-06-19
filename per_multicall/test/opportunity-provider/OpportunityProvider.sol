// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Structs.sol";
import "./Errors.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";
import "permit2/interfaces/ISignatureTransfer.sol";
import "openzeppelin-contracts/contracts/utils/ReentrancyGuard.sol";
import "@pythnetwork/express-relay-sdk-solidity/IExpressRelay.sol";

abstract contract OpportunityProvider is ReentrancyGuard {
    using SafeERC20 for IERC20;

    address _admin;
    address _permit2;
    address _expressRelay;

    string public constant _OPPORTUNITY_PROVIDER_WITNESS_TYPE =
        "OpportunityProviderWitness(TokenAmount[] buyTokens,address owner)TokenAmount(address token,uint256 amount)";
    string public constant _TOKEN_AMOUNT_TYPE =
        "TokenAmount(address token,uint256 amount)";

    string public constant OPPORTUNITY_PROVIDER_WITNESS_TYPE_STRING =
        "OpportunityProviderWitness witness)OpportunityProviderWitness(TokenAmount[] buyTokens,address owner)TokenAmount(address token,uint256 amount)TokenPermissions(address token,uint256 amount)";

    /**
     * @notice OpportunityProvider constructor - construct a new opportunity provider contract with given parameters
     *
     * @param admin: address of admin of opportunity provider
     * @param expressRelay: address of the express relay
     * @param permit2: address of permit2 of opportunity provider
     */
    constructor(address admin, address expressRelay, address permit2) {
        _admin = admin;
        _expressRelay = expressRelay;
        _permit2 = permit2;
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
                    keccak256(bytes(_OPPORTUNITY_PROVIDER_WITNESS_TYPE)),
                    hash(params.buyTokens),
                    params.owner
                )
            );
    }

    function _verifyParams(
        ExecutionParams calldata params,
        bytes calldata signature
    ) internal view {
        if (params.witness.owner != _admin) {
            revert Unauthorized();
        }
        if (
            !IExpressRelay(_expressRelay).isPermissioned(
                _expressRelay,
                signature
            )
        ) {
            revert InvalidOpportunity();
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

    function _transferSellTokens(
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
            transferDetails[i] = ISignatureTransfer.SignatureTransferDetails({
                to: msg.sender,
                requestedAmount: amount
            });
        }
        ISignatureTransfer(_permit2).permitWitnessTransferFrom(
            permit,
            transferDetails,
            witness.owner,
            hash(witness),
            OPPORTUNITY_PROVIDER_WITNESS_TYPE_STRING,
            signature
        );
    }

    function _transferBuyTokens(ExecutionWitness calldata witness) internal {
        for (uint i = 0; i < witness.buyTokens.length; i++) {
            IERC20 token = IERC20(witness.buyTokens[i].token);
            token.safeTransferFrom(
                msg.sender,
                witness.owner,
                witness.buyTokens[i].amount
            );
        }
    }

    function execute(
        ExecutionParams calldata params,
        bytes calldata signature
    ) public nonReentrant {
        _verifyParams(params, signature);
        _transferSellTokens(params.permit, params.witness, signature);
        _transferBuyTokens(params.witness);
    }
}

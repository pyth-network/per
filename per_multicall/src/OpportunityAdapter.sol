// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Structs.sol";
import "./SigVerify.sol";
import "./ExpressRelay.sol";
import "./WETH9.sol";

import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";

abstract contract OpportunityAdapter is SigVerify {
    address _admin;
    address _expressRelay;
    address _weth;
    mapping(bytes => bool) _signatureUsed;

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

    function getWethContract() internal view returns (WETH9) {
        return WETH9(payable(_weth));
    }

    function _verifyParams(ExecutionParams calldata params) internal view {
        if (msg.sender != _expressRelay) {
            revert Unauthorized();
        }

        bool validSignature = verifyCalldata(
            params.executor,
            abi.encode(
                params.sellTokens,
                params.buyTokens,
                params.targetContract,
                params.targetCalldata,
                params.targetCallValue,
                params.bidAmount,
                params.validUntil
            ),
            params.signature
        );
        if (!validSignature) {
            revert InvalidExecutorSignature();
        }

        _revertOnDuplicate(params.sellTokens);
        _revertOnDuplicate(params.buyTokens);

        if (block.timestamp > params.validUntil) {
            revert ExpiredSignature();
        }
        if (_signatureUsed[params.signature]) {
            revert SignatureAlreadyUsed();
        }
    }

    function _revertOnDuplicate(TokenAmount[] calldata tokens) internal pure {
        for (uint i = 0; i < tokens.length; i++) {
            for (uint j = i + 1; j < tokens.length; j++) {
                if (tokens[i].token == tokens[j].token) {
                    revert DuplicateToken();
                }
            }
        }
    }

    function _prepareSellTokens(ExecutionParams calldata params) internal {
        for (uint i = 0; i < params.sellTokens.length; i++) {
            IERC20 token = IERC20(params.sellTokens[i].token);
            token.transferFrom(
                params.executor,
                address(this),
                params.sellTokens[i].amount
            );
            token.approve(params.targetContract, params.sellTokens[i].amount);
        }
    }

    function _transferFromAndUnwrapWeth(
        address source,
        uint256 amount
    ) internal {
        WETH9 weth = getWethContract();
        try weth.transferFrom(source, address(this), amount) {} catch {
            revert WethTransferFromFailed();
        }
        weth.withdraw(amount);
    }

    function _settleBid(ExecutionParams calldata params) internal {
        if (params.bidAmount > 0) {
            _transferFromAndUnwrapWeth(params.executor, params.bidAmount);
            payable(getExpressRelay()).transfer(params.bidAmount);
        }
    }

    function _callTargetContract(ExecutionParams calldata params) internal {
        (bool success, bytes memory returnData) = params.targetContract.call{
            value: params.targetCallValue
        }(params.targetCalldata);
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
        ExecutionParams calldata params,
        uint256[] memory buyTokensBalancesBeforeCall
    ) internal {
        for (uint i = 0; i < params.buyTokens.length; i++) {
            IERC20 token = IERC20(params.buyTokens[i].token);
            if (
                token.balanceOf(address(this)) <
                buyTokensBalancesBeforeCall[i] + params.buyTokens[i].amount
            ) {
                revert InsufficientTokenReceived();
            }
            token.transfer(params.executor, params.buyTokens[i].amount);
        }
    }

    function executeOpportunity(
        ExecutionParams calldata params
    ) public payable {
        _verifyParams(params);
        // get balances of buy tokens before transferring sell tokens since there might be overlaps
        uint256[]
            memory buyTokensBalancesBeforeCall = _getContractTokenBalances(
                params.buyTokens
            );
        _prepareSellTokens(params);
        if (params.targetCallValue > 0) {
            _transferFromAndUnwrapWeth(params.executor, params.targetCallValue);
        }
        _callTargetContract(params);
        _validateAndTransferBuyTokens(params, buyTokensBalancesBeforeCall);
        _settleBid(params);
        _signatureUsed[params.signature] = true;
    }

    // necessary to receive ETH from WETH contract using withdraw
    receive() external payable {}
}

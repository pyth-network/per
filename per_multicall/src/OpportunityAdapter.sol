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
    string constant _EXECUTION_PARAMS_TYPE =
        "ExecutionParams(TokenAmount[] sellTokens,TokenAmount[] buyTokens,address executor,address targetContract,bytes targetCalldata,uint256 targetCallValue,uint256 validUntil,uint256 bidAmount)TokenAmount(address token,uint256 amount)";
    string constant _TOKEN_AMOUNT_TYPE =
        "TokenAmount(address token,uint256 amount)";
    string constant _DOMAIN_NAME = "OpportunityAdapter";
    string constant _DOMAIN_VERSION = "1";

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
        __EIP712_init(_DOMAIN_NAME, _DOMAIN_VERSION);
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

    function _getRevertMsg(
        bytes memory _returnData
    ) internal pure returns (string memory) {
        // If the _res length is less than 68, then the transaction failed silently (without a revert message)
        if (_returnData.length < 68) return "Transaction reverted silently";

        assembly {
            // Slice the sighash.
            _returnData := add(_returnData, 0x04)
        }
        return abi.decode(_returnData, (string)); // All that remains is the revert string
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

    function hash(ExecutionParams memory params) public pure returns (bytes32) {
        return
            keccak256(
                abi.encode(
                    keccak256(bytes(_EXECUTION_PARAMS_TYPE)),
                    hash(params.sellTokens),
                    hash(params.buyTokens),
                    params.executor,
                    params.targetContract,
                    keccak256(params.targetCalldata),
                    params.targetCallValue,
                    params.validUntil,
                    params.bidAmount
                )
            );
    }

    function _verifyParams(
        ExecutionParams memory params,
        bytes memory signature
    ) internal view {
        if (msg.sender != _expressRelay) {
            revert Unauthorized();
        }

        verifyCalldata(
            hash(params),
            params.executor,
            signature,
            params.validUntil
        );
    }

    function _prepareSellTokens(ExecutionParams memory params) internal {
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
        WETH9 weth = _getWethContract();
        if (amount > 0) {
            try weth.transferFrom(source, address(this), amount) {} catch {
                revert WethTransferFromFailed();
            }
            weth.withdraw(amount);
        }
    }

    function _settleBid(ExecutionParams memory params) internal {
        _transferFromAndUnwrapWeth(params.executor, params.bidAmount);
        payable(getExpressRelay()).transfer(params.bidAmount);
    }

    function _callTargetContract(ExecutionParams memory params) internal {
        (bool success, bytes memory reason) = params.targetContract.call{
            value: params.targetCallValue
        }(params.targetCalldata);
        if (!success) {
            string memory revertData = _getRevertMsg(reason);
            revert TargetCallFailed(revertData);
        }
    }

    function _getContractTokenBalances(
        TokenAmount[] memory tokens
    ) internal view returns (uint256[] memory) {
        uint256[] memory tokenBalances = new uint256[](tokens.length);
        for (uint i = 0; i < tokens.length; i++) {
            IERC20 token = IERC20(tokens[i].token);
            tokenBalances[i] = token.balanceOf(address(this));
        }
        return tokenBalances;
    }

    function _validateAndTransferBuyTokens(
        ExecutionParams memory params,
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
        ExecutionParams memory params,
        bytes memory signature
    ) public payable {
        _verifyParams(params, signature);
        // get balances of buy tokens before transferring sell tokens since there might be overlaps
        uint256[]
            memory buyTokensBalancesBeforeCall = _getContractTokenBalances(
                params.buyTokens
            );
        _prepareSellTokens(params);
        _transferFromAndUnwrapWeth(params.executor, params.targetCallValue);
        _callTargetContract(params);
        _validateAndTransferBuyTokens(params, buyTokensBalancesBeforeCall);
        _settleBid(params);
        _useSignature(signature);
    }

    receive() external payable {}
}

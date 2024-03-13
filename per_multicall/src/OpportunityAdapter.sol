// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Errors.sol";
import "./Structs.sol";
import "./ExpressRelayFeeReceiver.sol";
import "./SigVerify.sol";
import "./ExpressRelay.sol";
import "./WETH9.sol";

import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "forge-std/console.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";

contract OpportunityAdapter is SigVerify {
    address _expressRelay;
    address _weth;
    mapping(bytes => bool) _signatureUsed;

    /**
     * @notice OpportunityAdapter constructor - Initializes a new opportunity adapter contract with given parameters
     *
     * @param expressRelay: address of express relay
     * @param weth: address of WETH contract
     */
    constructor(address expressRelay, address weth) {
        _expressRelay = expressRelay;
        _weth = weth;
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

    function executeOpportunity(ExecutionParams memory params) public payable {
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
                params.value,
                params.bidAmount,
                params.validUntil
            ),
            params.signature
        );
        if (!validSignature) {
            revert InvalidSearcherSignature();
        }
        if (block.timestamp > params.validUntil) {
            revert ExpiredSignature();
        }
        if (_signatureUsed[params.signature]) {
            revert SignatureAlreadyUsed();
        }

        uint256[] memory balancesBuyTokens = new uint256[](
            params.buyTokens.length
        );

        address weth = getWeth();
        // transfer sell tokens to this contract
        for (uint i = 0; i < params.sellTokens.length; i++) {
            IERC20 token = IERC20(params.sellTokens[i].token);

            token.transferFrom(
                params.executor,
                address(this),
                params.sellTokens[i].amount
            );

            // approve contract to spend sell tokens
            uint256 approveAmount = params.sellTokens[i].amount;
            if (params.sellTokens[i].token == weth) {
                if (approveAmount >= params.value) {
                    // we need `parmas.value` of to be sent to the contract directly
                    // so this amount should be subtracted from the approveAmount
                    approveAmount = approveAmount - params.value;
                } else {
                    revert InsufficientWETHForMsgValue();
                }
            }
            token.approve(params.targetContract, approveAmount);
        }

        // get balances of buy tokens before call
        for (uint i = 0; i < params.buyTokens.length; i++) {
            IERC20 token = IERC20(params.buyTokens[i].token);
            uint256 amount = params.buyTokens[i].amount;

            balancesBuyTokens[i] = token.balanceOf(address(this)) + amount;
        }
        if (params.value > 0) {
            // unwrap weth to eth to use in call
            // TODO: Wrap in try catch and throw a revert with a better error since WETH9 reverts do not return a reason
            WETH9(payable(weth)).withdraw(params.value);
        }

        (bool success, bytes memory reason) = params.targetContract.call{
            value: params.value
        }(params.targetCalldata);

        if (!success) {
            string memory revertData = _getRevertMsg(reason);
            revert TargetCallFailed(revertData);
        }

        // check balances of buy tokens after call and transfer to opportunity adapter
        for (uint i = 0; i < params.buyTokens.length; i++) {
            IERC20 token = IERC20(params.buyTokens[i].token);
            uint256 amount = params.buyTokens[i].amount;

            uint256 balanceFinal = token.balanceOf(address(this));
            if (balanceFinal < balancesBuyTokens[i]) {
                revert InsufficientTokenReceived();
            }

            // transfer buy tokens to the executor
            token.transfer(params.executor, amount);
        }

        // transfer bid to opportunity adapter in the form of weth
        WETH9(payable(weth)).transferFrom(
            params.executor,
            address(this),
            params.bidAmount
        );
        // unwrap weth to eth
        WETH9(payable(weth)).withdraw(params.bidAmount);
        payable(getExpressRelay()).transfer(params.bidAmount);

        // mark signature as used
        _signatureUsed[params.signature] = true;
    }

    receive() external payable {} // TODO: can we get rid of this? seems not but unsure why
}

// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./Errors.sol";
import "./Structs.sol";
import "./PERFeeReceiver.sol";
import "./SigVerify.sol";
import "./PERMulticall.sol";
import "./WETH9.sol";

import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "forge-std/console.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";

contract LiquidationAdapter is SigVerify {
    address _perMulticall;
    address _weth;
    mapping(bytes => bool) _signatureUsed;

    /**
     * @notice LiquidationAdapter constructor - Initializes a new liquidation adapter contract with given parameters
     *
     * @param perMulticall: address of PER multicall
     * @param weth: address of WETH contract
     */
    constructor(address perMulticall, address weth) {
        _perMulticall = perMulticall;
        _weth = weth;
    }

    /**
     * @notice getPERMulticall function - returns the address of the PER multicall
     */
    function getPERMulticall() public view returns (address) {
        return _perMulticall;
    }

    /**
     * @notice getWeth function - returns the address of the WETH contract used by multicall
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

    function callLiquidation(
        LiquidationCallParams memory params
    ) public payable {
        if (msg.sender != _perMulticall) {
            revert Unauthorized();
        }

        bool validSignature = verifyCalldata(
            params.liquidator,
            abi.encode(
                params.repayTokens,
                params.expectedReceiptTokens,
                params.contractAddress,
                params.data,
                params.value,
                params.bid
            ),
            params.validUntil,
            params.signatureLiquidator
        );
        if (!validSignature) {
            revert InvalidSearcherSignature();
        }
        if (block.number > params.validUntil) {
            revert ExpiredSignature();
        }
        if (_signatureUsed[params.signatureLiquidator]) {
            revert SignatureAlreadyUsed();
        }

        uint256[] memory balancesExpectedReceipt = new uint256[](
            params.expectedReceiptTokens.length
        );

        address weth = getWeth();
        // transfer repay tokens to this contract
        for (uint i = 0; i < params.repayTokens.length; i++) {
            IERC20 token = IERC20(params.repayTokens[i].token);

            token.transferFrom(
                params.liquidator,
                address(this),
                params.repayTokens[i].amount
            );

            // approve contract to spend repay tokens
            uint256 approveAmount = params.repayTokens[i].amount;
            if (params.repayTokens[i].token == weth) {
                if (approveAmount >= params.value) {
                    // we need `parmas.value` of to be sent to the contract directly
                    // so this amount should be subtracted from the approveAmount
                    approveAmount = approveAmount - params.value;
                } else {
                    revert InsufficientWETHForMsgValue();
                }
            }
            token.approve(params.contractAddress, approveAmount);
        }

        // get balances of receipt tokens before call
        for (uint i = 0; i < params.expectedReceiptTokens.length; i++) {
            IERC20 token = IERC20(params.expectedReceiptTokens[i].token);
            uint256 amount = params.expectedReceiptTokens[i].amount;

            balancesExpectedReceipt[i] =
                token.balanceOf(address(this)) +
                amount;
        }
        if (params.value > 0) {
            // unwrap weth to eth to use in call
            WETH9(payable(weth)).withdraw(params.value);
        }

        (bool success, bytes memory reason) = params.contractAddress.call{
            value: params.value
        }(params.data);

        if (!success) {
            string memory revertData = _getRevertMsg(reason);
            revert LiquidationCallFailed(revertData);
        }

        // check balances of receipt tokens after call and transfer to liquidator
        for (uint i = 0; i < params.expectedReceiptTokens.length; i++) {
            IERC20 token = IERC20(params.expectedReceiptTokens[i].token);
            uint256 amount = params.expectedReceiptTokens[i].amount;

            uint256 balanceFinal = token.balanceOf(address(this));
            require(
                balanceFinal >= balancesExpectedReceipt[i],
                "insufficient token received"
            );

            // transfer receipt tokens to liquidator
            token.transfer(params.liquidator, amount);
        }

        // transfer bid to PER adapter in the form of weth
        WETH9(payable(weth)).transferFrom(
            params.liquidator,
            address(this),
            params.bid
        );
        // unwrap weth to eth
        WETH9(payable(weth)).withdraw(params.bid);
        // transfer eth to PER multicall
        payable(getPERMulticall()).transfer(params.bid);

        // mark signature as used
        _signatureUsed[params.signatureLiquidator] = true;
    }

    receive() external payable {} // TODO: can we get rid of this? seems not but unsure why
}

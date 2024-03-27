// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Errors.sol";
import "./Structs.sol";
import "./ExpressRelayState.sol";
import "./ExpressRelayHelpers.sol";

import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "@pythnetwork/express-relay-sdk-solidity/IExpressRelayFeeReceiver.sol";

contract ExpressRelay is ExpressRelayHelpers, ExpressRelayState {
    event ReceivedETH(address sender, uint256 amount);

    /**
     * @notice ExpressRelay constructor - Initializes a new ExpressRelay contract with given parameters
     *
     * @param admin: address of admin of express relay
     * @param relayer: address of relayer EOA
     * @param feeSplitProtocolDefault: default fee split to be paid to the protocol whose permissioning is being used
     * @param feeSplitRelayer: split of the non-protocol fees to be paid to the relayer
     */
    constructor(
        address admin,
        address relayer,
        uint256 feeSplitProtocolDefault,
        uint256 feeSplitRelayer
    ) {
        state.admin = admin;
        state.relayer = relayer;

        validateFeeSplit(feeSplitProtocolDefault);
        state.feeSplitProtocolDefault = feeSplitProtocolDefault;

        validateFeeSplit(feeSplitRelayer);
        state.feeSplitRelayer = feeSplitRelayer;
    }

    function multicallPermissionKey(
        MulticallData calldata multicallData
    )
        internal
        returns (
            MulticallStatus[] memory multicallStatusesPermissionKey,
            uint256 totalBid,
            uint256 feeProtocol
        )
    {
        bytes memory permissionKey = multicallData.permissionKey;
        CallWithBidData[] memory callWithBidData = multicallData.data;

        if (permissionKey.length < 20) {
            revert InvalidPermission();
        }

        state.permissions[keccak256(permissionKey)] = true;
        multicallStatusesPermissionKey = new MulticallStatus[](
            callWithBidData.length
        );

        totalBid = 0;
        for (uint256 j = 0; j < callWithBidData.length; j++) {
            try
                // callWithBid will revert if call to external contract fails or if bid conditions not met
                this.callWithBid(callWithBidData[j])
            returns (bool success, bytes memory result) {
                multicallStatusesPermissionKey[j].externalSuccess = success;
                multicallStatusesPermissionKey[j].externalResult = result;
            } catch Error(string memory reason) {
                multicallStatusesPermissionKey[j]
                    .multicallRevertReason = reason;
            }

            // only count bid if call was successful (and bid was paid out)
            if (multicallStatusesPermissionKey[j].externalSuccess) {
                totalBid += callWithBidData[j].bidAmount;
            }
        }

        // use the first 20 bytes of permission as fee receiver
        address feeReceiver = bytesToAddress(permissionKey);
        // transfer fee to the protocol
        uint256 feeSplitProtocol = state.feeConfig[feeReceiver];
        if (feeSplitProtocol == 0) {
            feeSplitProtocol = state.feeSplitProtocolDefault;
        }
        feeProtocol = (totalBid * feeSplitProtocol) / state.feeSplitPrecision;
        if (feeProtocol > 0) {
            if (isContract(feeReceiver)) {
                IExpressRelayFeeReceiver(feeReceiver).receiveAuctionProceedings{
                    value: feeProtocol
                }(permissionKey);
            } else {
                payable(feeReceiver).transfer(feeProtocol);
            }
        }
        state.permissions[keccak256(permissionKey)] = false;
    }

    /**
     * @notice multicall function - performs a number of calls to external contracts in order
     *
     * @param multicallData: ordered list of data for multicall; each entry in list consists of a permission key and a list of data representing individual searchers' calls
     */
    function multicall(
        MulticallData[] calldata multicallData
    )
        public
        payable
        onlyRelayer
        returns (MulticallStatus[][] memory multicallStatuses)
    {
        uint256 totalFees = 0;
        uint256 totalFeesProtocol = 0;
        multicallStatuses = new MulticallStatus[][](multicallData.length);

        for (uint256 i = 0; i < multicallData.length; i++) {
            (
                MulticallStatus[] memory multicallStatusesPermissionKey,
                uint256 totalBid,
                uint256 feeProtocol
            ) = multicallPermissionKey(multicallData[i]);
            multicallStatuses[i] = multicallStatusesPermissionKey;
            totalFees += totalBid;
            totalFeesProtocol += feeProtocol;
            // bytes memory permissionKey = multicallData[i].permissionKey;
            // CallWithBidData[] memory callWithBidData = multicallData[i].data;

            // if (permissionKey.length < 20) {
            //     revert InvalidPermission();
            // }

            // state.permissions[keccak256(permissionKey)] = true;
            // MulticallStatus[] memory multicallStatusesPermissionKey = new MulticallStatus[](callWithBidData.length);

            // uint256 totalBid = 0;
            // for (uint256 j = 0; j < callWithBidData.length; j++) {
            //     try
            //         // callWithBid will revert if call to external contract fails or if bid conditions not met
            //         this.callWithBid(callWithBidData[j])
            //     returns (bool success, bytes memory result) {
            //         multicallStatusesPermissionKey[j].externalSuccess = success;
            //         multicallStatusesPermissionKey[j].externalResult = result;
            //     } catch Error(string memory reason) {
            //         multicallStatusesPermissionKey[j].multicallRevertReason = reason;
            //     }

            //     // only count bid if call was successful (and bid was paid out)
            //     if (multicallStatusesPermissionKey[j].externalSuccess) {
            //         totalBid += callWithBidData[j].bidAmount;
            //     }
            // }
            // multicallStatuses[i] = multicallStatusesPermissionKey;
            // totalFees += totalBid;

            // // use the first 20 bytes of permission as fee receiver
            // address feeReceiver = bytesToAddress(permissionKey);
            // // transfer fee to the protocol
            // uint256 feeSplitProtocol = state.feeConfig[feeReceiver];
            // if (feeSplitProtocol == 0) {
            //     feeSplitProtocol = state.feeSplitProtocolDefault;
            // }
            // uint256 feeProtocol = (totalBid * feeSplitProtocol) /
            //     state.feeSplitPrecision;
            // if (feeProtocol > 0) {
            //     if (isContract(feeReceiver)) {
            //         IExpressRelayFeeReceiver(feeReceiver).receiveAuctionProceedings{
            //             value: feeProtocol
            //         }(permissionKey);
            //     } else {
            //         payable(feeReceiver).transfer(feeProtocol);
            //     }
            //     totalFeesProtocol += feeProtocol;
            // }
            // state.permissions[keccak256(permissionKey)] = false;
        }

        // pay the relayer
        if (totalFees > totalFeesProtocol) {
            uint256 feeRelayer = ((totalFees - totalFeesProtocol) *
                state.feeSplitRelayer) / state.feeSplitPrecision;
            if (feeRelayer > 0) {
                payable(state.relayer).transfer(feeRelayer);
            }
        }
    }

    /**
     * @notice callWithBid function - contained call to function with check for bid invariant
     *
     * @param callWithBidData: the targetContract and targetCalldata for the external call, and the bidAmount to be paid
     */
    function callWithBid(
        CallWithBidData calldata callWithBidData
    ) public payable returns (bool, bytes memory) {
        // manual check for internal call (function is public for try/catch)
        if (msg.sender != address(this)) {
            revert Unauthorized();
        }

        uint256 balanceInitEth = address(this).balance;

        (bool success, bytes memory result) = callWithBidData
            .targetContract
            .call(callWithBidData.targetCalldata);

        if (success) {
            uint256 balanceFinalEth = address(this).balance;

            // ensure that this contract was paid at least bid ETH
            require(
                (balanceFinalEth - balanceInitEth >=
                    callWithBidData.bidAmount) &&
                    (balanceFinalEth >= balanceInitEth),
                "invalid bid"
            );
        }

        return (success, result);
    }

    receive() external payable {
        emit ReceivedETH(msg.sender, msg.value);
    }
}

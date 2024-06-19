// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import "./Errors.sol";
import "./Structs.sol";
import "./State.sol";
import "./Helpers.sol";
import "./Events.sol";

import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "openzeppelin-contracts/contracts/utils/ReentrancyGuard.sol";
import "@pythnetwork/express-relay-sdk-solidity/IExpressRelayFeeReceiver.sol";
import "ExcessivelySafeCall/ExcessivelySafeCall.sol";

contract ExpressRelay is Helpers, State, ExpressRelayEvents, ReentrancyGuard {
    using ExcessivelySafeCall for address;

    /**
     * @notice ExpressRelay initializer - Initializes a new ExpressRelay contract with given parameters
     *
     * @param admin: address of admin of express relay
     * @param relayer: address of relayer EOA
     * @param feeSplitProtocolDefault: default fee split to be paid to the protocol whose permissioning is being used
     * @param feeSplitRelayer: split of the non-protocol fees to be paid to the relayer
     */
    function _initialize(
        address admin,
        address relayer,
        uint256 feeSplitProtocolDefault,
        uint256 feeSplitRelayer
    ) internal {
        state.admin = admin;
        state.relayer = relayer;
        state.relayerSubwallets = new address[](0);

        setFeeSplitPrecision();

        validateFeeSplit(feeSplitProtocolDefault);
        state.feeSplitProtocolDefault = feeSplitProtocolDefault;

        validateFeeSplit(feeSplitRelayer);
        state.feeSplitRelayer = feeSplitRelayer;
    }

    /**
     * @notice multicall function - performs a number of calls to external contracts in order
     *
     * @param permissionKey: permission to allow for this call
     * @param multicallData: ordered list of data for multicall, consisting of targetContract, targetCalldata, and bidAmount
     */
    function multicall(
        bytes calldata permissionKey,
        MulticallData[] calldata multicallData
    )
        public
        payable
        onlyRelayer
        nonReentrant
        returns (MulticallStatus[] memory multicallStatuses)
    {
        if (permissionKey.length < 20) {
            revert InvalidPermission();
        }

        state.permissions[keccak256(permissionKey)] = true;
        multicallStatuses = new MulticallStatus[](multicallData.length);

        uint256 totalBid = 0;
        for (uint256 i = 0; i < multicallData.length; i++) {
            try
                // callWithBid will revert if call to external contract fails or if bid conditions not met
                this.callWithBid(multicallData[i])
            returns (bool success, bytes memory result) {
                multicallStatuses[i].externalSuccess = success;
                multicallStatuses[i].externalResult = result;
            } catch Error(string memory reason) {
                multicallStatuses[i].multicallRevertReason = reason;
            }

            // only count bid if call was successful (and bid was paid out)
            if (multicallStatuses[i].externalSuccess) {
                totalBid += multicallData[i].bidAmount;
            }

            emit MulticallIssued(
                permissionKey,
                i,
                multicallData[i].bidId,
                multicallData[i].bidAmount,
                multicallStatuses[i]
            );
        }

        // use the first 20 bytes of permission as fee receiver
        address feeReceiver = bytesToAddress(permissionKey);
        // transfer fee to the protocol
        uint256 feeSplitProtocol = state.feeConfig[feeReceiver];
        if (feeSplitProtocol == 0) {
            feeSplitProtocol = state.feeSplitProtocolDefault;
        }
        uint256 feeProtocol = (totalBid * feeSplitProtocol) /
            state.feeSplitPrecision;
        if (feeProtocol > 0) {
            if (isContract(feeReceiver)) {
                IExpressRelayFeeReceiver(feeReceiver).receiveAuctionProceedings{
                    value: feeProtocol
                }(permissionKey);
            } else {
                (bool sent, ) = feeReceiver.call{value: feeProtocol}("");
                require(sent, "Fee transfer to protocol failed");
            }
        }
        state.permissions[keccak256(permissionKey)] = false;

        // pay the relayer
        uint256 feeRelayer = ((totalBid - feeProtocol) *
            state.feeSplitRelayer) / state.feeSplitPrecision;
        if (feeRelayer > 0) {
            (bool sent, ) = state.relayer.call{value: feeRelayer}("");
            require(sent, "Fee transfer to relayer failed");
        }
    }

    /**
     * @notice callWithBid function - contained call to function with check for bid invariant
     *
     * @param multicallData: data for multicall, consisting of targetContract, targetCalldata, and bidAmount
     */
    function callWithBid(
        MulticallData calldata multicallData
    ) public payable returns (bool, bytes memory) {
        // manual check for internal call (function is public for try/catch)
        if (msg.sender != address(this)) {
            revert Unauthorized();
        }

        uint256 balanceInitEth = address(this).balance;

        (bool success, bytes memory result) = multicallData
            .targetContract
            .excessivelySafeCall(
                gasleft(), // this will automatically forward 63/64 of gas
                0,
                32,
                multicallData.targetCalldata
            );

        if (success) {
            uint256 balanceFinalEth = address(this).balance;

            // ensure that this contract was paid at least bid ETH
            require(
                (balanceFinalEth - balanceInitEth >= multicallData.bidAmount),
                "invalid bid"
            );
        }

        return (success, result);
    }

    receive() external payable {
        emit ReceivedETH(msg.sender, msg.value);
    }
}

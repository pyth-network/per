// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Errors.sol";
import "./Structs.sol";
import "./ExpressRelayState.sol";

import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "@pythnetwork/express-relay-sdk-solidity/IExpressRelay.sol";
import "@pythnetwork/express-relay-sdk-solidity/IExpressRelayFeeReceiver.sol";

contract ExpressRelay is IExpressRelay, ExpressRelayState {
    event ReceivedETH(address sender, uint256 amount);

    /**
     * @notice ExpressRelay constructor - Initializes a new multicall contract with given parameters
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
        // TODO: move this to ExpressRelayState?
        state.feeSplitPrecision = 10 ** 18;

        state.admin = admin;
        state.relayer = relayer;

        if (feeSplitProtocolDefault > state.feeSplitPrecision) {
            revert InvalidFeeSplit();
        }
        state.feeSplitProtocolDefault = feeSplitProtocolDefault;

        if (feeSplitRelayer > state.feeSplitPrecision) {
            revert InvalidFeeSplit();
        }
        state.feeSplitRelayer = feeSplitRelayer;
    }

    // TODO: move this to ExpressRelayState/Setters/Getters?
    /**
     * @notice setRelayer function - sets the relayer
     *
     * @param relayer: address of the relayer to be set
     */
    function setRelayer(address relayer) public onlyAdmin {
        state.relayer = relayer;
    }

    /**
     * @notice getRelayer function - returns the address of the relayer
     */
    function getRelayer() public view returns (address) {
        return state.relayer;
    }

    /**
     * @notice setFeeProtocolDefault function - sets the default fee split for the protocol
     *
     * @param feeSplit: split of fee to be sent to the protocol. 10**18 is 100%
     */
    function setFeeProtocolDefault(uint256 feeSplit) public onlyAdmin {
        if (feeSplit > state.feeSplitPrecision) {
            revert InvalidFeeSplit();
        }
        state.feeSplitProtocolDefault = feeSplit;
    }

    /**
     * @notice getFeeProtocolDefault function - returns the default fee split for the protocol
     */
    function getFeeProtocolDefault() public view returns (uint256) {
        return state.feeSplitProtocolDefault;
    }

    /**
     * @notice setFeeProtocol function - sets the fee split for a given protocol fee recipient
     *
     * @param feeRecipient: address of the fee recipient for the protocol
     * @param feeSplit: split of fee to be sent to the protocol. 10**18 is 100%
     */
    function setFeeProtocol(
        address feeRecipient,
        uint256 feeSplit
    ) public onlyAdmin {
        if (feeSplit > state.feeSplitPrecision) {
            revert InvalidFeeSplit();
        }
        state.feeConfig[feeRecipient] = feeSplit;
    }

    /**
     * @notice getFeeProtocol function - returns the fee split for a given protocol fee recipient
     *
     * @param feeRecipient: address of the fee recipient for the protocol
     */
    function getFeeProtocol(
        address feeRecipient
    ) public view returns (uint256) {
        uint256 feeProtocol = state.feeConfig[feeRecipient];
        if (feeProtocol == 0) {
            feeProtocol = state.feeSplitProtocolDefault;
        }
        return feeProtocol;
    }

    /**
     * @notice setFeeRelayer function - sets the fee split for the relayer
     *
     * @param feeSplit: split of remaining fee (after subtracting protocol fee) to be sent to the relayer. 10**18 is 100%
     */
    function setFeeRelayer(uint256 feeSplit) public onlyAdmin {
        if (feeSplit > state.feeSplitPrecision) {
            revert InvalidFeeSplit();
        }
        state.feeSplitRelayer = feeSplit;
    }

    /**
     * @notice getFeeRelayer function - returns the fee split for the relayer
     */
    function getFeeRelayer() public view returns (uint256) {
        return state.feeSplitRelayer;
    }

    function isPermissioned(
        address protocolFeeReceiver,
        bytes calldata permissionId
    ) public view returns (bool permissioned) {
        return
            state.permissions[
                keccak256(abi.encode(protocolFeeReceiver, permissionId))
            ];
    }

    function _isContract(address _addr) private view returns (bool) {
        uint32 size;
        assembly {
            size := extcodesize(_addr)
        }
        return (size > 0);
    }

    function _bytesToAddress(
        bytes memory bys
    ) private pure returns (address addr) {
        // this does not assume the struct fields of the permission key
        addr = address(uint160(uint256(bytes32(bys))));
    }

    /**
     * @notice multicall function - performs a number of calls to external contracts in order
     *
     * @param permissionKey: permission to allow for this call
     * @param targetContracts: ordered list of contracts to call into
     * @param targetCalldata: ordered list of calldata to call the targets with
     * @param bidAmounts: ordered list of bids; call i will fail if it does not send this contract at least bid i
     */
    function multicall(
        bytes calldata permissionKey,
        address[] calldata targetContracts,
        bytes[] calldata targetCalldata,
        uint256[] calldata bidAmounts
    )
        public
        payable
        onlyRelayer
        returns (MulticallStatus[] memory multicallStatuses)
    {
        if (permissionKey.length < 20) {
            revert InvalidPermission();
        }

        state.permissions[keccak256(permissionKey)] = true;
        multicallStatuses = new MulticallStatus[](targetCalldata.length);

        uint256 totalBid = 0;
        for (uint256 i = 0; i < targetCalldata.length; i++) {
            // try/catch will revert if call to searcher fails or if bid conditions not met
            try
                this.callWithBid(
                    targetContracts[i],
                    targetCalldata[i],
                    bidAmounts[i]
                )
            returns (bool success, bytes memory result) {
                multicallStatuses[i].externalSuccess = success;
                multicallStatuses[i].externalResult = result;
            } catch Error(string memory reason) {
                multicallStatuses[i].multicallRevertReason = reason;
            }

            // only count bid if call was successful (and bid was paid out)
            if (multicallStatuses[i].externalSuccess) {
                totalBid += bidAmounts[i];
            }
        }

        // use the first 20 bytes of permission as fee receiver
        address feeReceiver = _bytesToAddress(permissionKey);
        // transfer fee to the protocol
        uint256 feeSplitProtocol = state.feeConfig[feeReceiver];
        if (feeSplitProtocol == 0) {
            feeSplitProtocol = state.feeSplitProtocolDefault;
        }
        uint256 feeProtocol;
        uint256 feeProtocolNumerator = totalBid * feeSplitProtocol;
        if (feeProtocolNumerator > 0) {
            feeProtocol = feeProtocolNumerator / state.feeSplitPrecision;
            if (_isContract(feeReceiver)) {
                IExpressRelayFeeReceiver(feeReceiver).receiveAuctionProceedings{
                    value: feeProtocol
                }(permissionKey);
            } else {
                payable(feeReceiver).transfer(feeProtocol);
            }
        }
        state.permissions[keccak256(permissionKey)] = false;

        // pay the relayer
        uint256 feeRelayerNumerator = (totalBid - feeProtocol) *
            state.feeSplitRelayer;
        if (feeRelayerNumerator > 0) {
            uint256 feeRelayer = feeRelayerNumerator / state.feeSplitPrecision;
            payable(state.relayer).transfer(feeRelayer);
        }
    }

    /**
     * @notice callWithBid function - contained call to function with check for bid invariant
     *
     * @param targetContract: contract address to call into
     * @param targetCalldata: calldata to call the target with
     * @param bid: bid to be paid; call will fail if it does not send this contract at least bid,
     */
    function callWithBid(
        address targetContract,
        bytes calldata targetCalldata,
        uint256 bid
    ) public payable returns (bool, bytes memory) {
        uint256 balanceInitEth = address(this).balance;

        (bool success, bytes memory result) = targetContract.call(
            targetCalldata
        );

        if (success) {
            uint256 balanceFinalEth = address(this).balance;

            // ensure that this contract was paid at least bid ETH
            require(
                (balanceFinalEth - balanceInitEth >= bid) &&
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

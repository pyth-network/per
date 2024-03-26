// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Errors.sol";
import "./Structs.sol";

import "@pythnetwork/express-relay-sdk-solidity/IExpressRelay.sol";

contract ExpressRelayStorage {
    struct State {
        // address of admin of express relay, handles setting fees and relayer role
        address admin;
        // address of relayer EOA uniquely permissioned to call ExpressRelay.multicall
        address relayer;
        // stores custom fee splits for protocol fee receivers
        mapping(address => uint256) feeConfig;
        // stores the flags for whether permission keys are currently allowed
        mapping(bytes32 => bool) permissions;
        // default fee split for protocol, used if custom fee split is not set
        uint256 feeSplitProtocolDefault;
        // split of the non-protocol fees to be paid to the relayer
        uint256 feeSplitRelayer;
        // precision for fee splits
        uint256 feeSplitPrecision;
    }
}

contract ExpressRelayState is IExpressRelay {
    ExpressRelayStorage.State state;

    constructor() {
        state.feeSplitPrecision = 10 ** 18;
    }

    modifier onlyAdmin() {
        if (msg.sender != state.admin) {
            revert Unauthorized();
        }
        _;
    }

    modifier onlyRelayer() {
        if (msg.sender != state.relayer) {
            revert Unauthorized();
        }
        _;
    }

    function validateFeeSplit(uint256 feeSplit) internal view {
        if (feeSplit > state.feeSplitPrecision) {
            revert InvalidFeeSplit();
        }
    }

    /**
     * @notice getAdmin function - returns the address of the admin
     */
    function getAdmin() public view returns (address) {
        return state.admin;
    }

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
        validateFeeSplit(feeSplit);
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
        validateFeeSplit(feeSplit);
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
        validateFeeSplit(feeSplit);
        state.feeSplitRelayer = feeSplit;
    }

    /**
     * @notice getFeeRelayer function - returns the fee split for the relayer
     */
    function getFeeRelayer() public view returns (uint256) {
        return state.feeSplitRelayer;
    }

    /**
     * @notice getFeeSplitPrecision function - returns the precision for fee splits
     */
    function getFeeSplitPrecision() public view returns (uint256) {
        return state.feeSplitPrecision;
    }

    /**
     * @notice isPermissioned function - checks if a given permission key is currently allowed
     *
     * @param protocolFeeReceiver: address of the protocol fee receiver, first part of permission key
     * @param permissionId: arbitrary bytes representing the action being gated, second part of the permission key
     */
    function isPermissioned(
        address protocolFeeReceiver,
        bytes calldata permissionId
    ) public view returns (bool permissioned) {
        return
            state.permissions[
                keccak256(abi.encode(protocolFeeReceiver, permissionId))
            ];
    }
}

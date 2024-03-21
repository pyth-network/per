// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Errors.sol";
import "./Structs.sol";

contract ExpressRelayStorage {
    struct State {
        // address of admin of express relay
        address admin;
        // address of relayer EOA
        address relayer;
        // custom fee splits for protocol fee receivers
        mapping(address => uint256) feeConfig;
        // permission key flag storage
        mapping(bytes32 => bool) permissions;
        // default fee split to be paid to the protocol whose permissioning is being used
        uint256 feeSplitProtocolDefault;
        // split of the non-protocol fees to be paid to the relayer
        uint256 feeSplitRelayer;
        // precision for fee splits
        uint256 feeSplitPrecision;
    }
}

contract ExpressRelayState {
    ExpressRelayStorage.State state;

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
}

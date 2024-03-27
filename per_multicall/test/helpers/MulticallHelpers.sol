// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {Test, console2} from "forge-std/Test.sol";
import "forge-std/console.sol";

import "./TestParsingHelpers.sol";

contract MulticallHelpers is Test, TestParsingHelpers {
    function assertFailedMulticall(
        MulticallStatus memory status,
        string memory reason
    ) internal {
        // assert the multicall revert reason matches the expected reason
        assertEq(status.multicallRevertReason, reason);
    }

    function assertFailedExternal(
        MulticallStatus memory status,
        bytes4 errorSelector
    ) internal {
        assertEq(bytes4(status.externalResult), errorSelector);
    }

    function logMulticallStatuses(
        MulticallStatus[] memory multicallStatuses
    ) internal view {
        for (uint256 i = 0; i < multicallStatuses.length; i++) {
            console.log("External Success:");
            console.log(multicallStatuses[i].externalSuccess);
            console.log("External Result:");
            console.logBytes(multicallStatuses[i].externalResult);
            console.log("Multicall Revert reason:");
            console.log(multicallStatuses[i].multicallRevertReason);
            console.log("----------------------------");
        }
    }
}

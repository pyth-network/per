// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {Test, console2} from "forge-std/Test.sol";
import "forge-std/console.sol";

import "./TestParsingHelpers.sol";

contract MulticallHelpers is Test, TestParsingHelpers {
    function assertFailedMulticall(
        string memory multicallRevertReason,
        string memory reason
    ) internal {
        // assert the multicall revert reason matches the expected reason
        assertEq(multicallRevertReason, reason);
    }

    function assertFailedExternal(
        bytes memory externalResult,
        bytes4 errorSelector
    ) internal {
        assertEq(bytes4(externalResult), errorSelector);
    }

    function logMulticallStatuses(
        bool[][] memory externalSuccesses,
        bytes[][] memory externalResults,
        string[][] memory multicallRevertReasons
    ) internal view {
        require(
            (externalSuccesses.length == externalResults.length) &&
                (externalResults.length == multicallRevertReasons.length),
            "arrays are not of equal length"
        );
        for (uint256 i = 0; i < externalSuccesses.length; i++) {
            require(
                (externalSuccesses[i].length == externalResults[i].length) &&
                    (externalResults[i].length ==
                        multicallRevertReasons[i].length),
                "inner arrays are not of equal length"
            );
            console.log("Multicall Statuses for call ", i);
            for (uint256 j = 0; j < externalSuccesses[i].length; j++) {
                console.log("External Success:");
                console.log(externalSuccesses[i][j]);
                console.log("External Result:");
                console.logBytes(externalResults[i][j]);
                console.log("Multicall Revert reason:");
                console.log(multicallRevertReasons[i][j]);
                console.log("----------------------------");
            }
            console.log("\n");
        }
    }
}

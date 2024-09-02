// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import {Test, console2} from "forge-std/Test.sol";
import "forge-std/console.sol";

import "./TestParsingHelpers.sol";
import "src/express-relay/Structs.sol";

contract GasVerifier {
    function verifyGas() public view {
        assert(gasleft() < 1000);
    }
}

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
        bytes memory errorSelector
    ) internal {
        assertEq(bytes4(status.externalResult), bytes4(errorSelector));
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

    function checkMulticallStatuses(
        MulticallStatus[] memory observed,
        MulticallStatus[] memory expected,
        bool checkExternalResult
    ) internal {
        assertEq(observed.length, expected.length);
        for (uint256 i = 0; i < observed.length; i++) {
            assertEq(observed[i].externalSuccess, expected[i].externalSuccess);

            if (checkExternalResult) {
                assertEq(
                    bytes4(observed[i].externalResult),
                    bytes4(expected[i].externalResult)
                );
            }

            assertEq(
                observed[i].multicallRevertReason,
                expected[i].multicallRevertReason
            );
        }
    }
}

// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Test, console2} from "forge-std/Test.sol";
import "forge-std/console.sol";

import {MyToken} from "../../src/MyToken.sol";
import "../../src/Structs.sol";

contract TestParsingHelpers is Test {
    struct BidInfo {
        uint256 bid;
        uint256 validUntil;
        address liquidator;
        uint256 liquidatorSk;
    }

    struct AccountBalance {
        uint256 collateral;
        uint256 debt;
    }

    function keccakHash(
        string memory functionInterface
    ) public pure returns (bytes memory) {
        return abi.encodePacked(bytes4(keccak256(bytes(functionInterface))));
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

    function extractBidAmounts(
        BidInfo[] memory bids
    ) public pure returns (uint256[] memory bidAmounts) {
        bidAmounts = new uint256[](bids.length);
        for (uint i = 0; i < bids.length; i++) {
            bidAmounts[i] = bids[i].bid;
        }
    }

    function makeBidInfo(
        uint256 bid,
        uint256 liquidatorSk
    ) internal pure returns (BidInfo memory) {
        return
            BidInfo(
                bid,
                1_000_000_000_000,
                vm.addr(liquidatorSk),
                liquidatorSk
            );
    }

    function assertEqBalances(
        AccountBalance memory a,
        AccountBalance memory b
    ) internal {
        assertEq(a.collateral, b.collateral);
        assertEq(a.debt, b.debt);
    }

    function getBalances(
        address account,
        address tokenCollateral,
        address tokenDebt
    ) public view returns (AccountBalance memory) {
        return
            AccountBalance(
                MyToken(tokenCollateral).balanceOf(account),
                MyToken(tokenDebt).balanceOf(account)
            );
    }

    function assertFailedMulticall(
        MulticallStatus memory status,
        string memory reason
    ) internal {
        // assert the multicall revert reason matches the expected reason
        assertEq(status.multicallRevertReason, reason);
    }

    function assertFailedExternal(
        MulticallStatus memory status,
        string memory reason
    ) internal {
        assertEq(
            abi.encodePacked(bytes4(status.externalResult)),
            keccakHash(reason)
        );
    }

    function compareStrings(
        string memory a,
        string memory b
    ) public pure returns (bool) {
        return keccak256(abi.encodePacked(a)) == keccak256(abi.encodePacked(b));
    }
}

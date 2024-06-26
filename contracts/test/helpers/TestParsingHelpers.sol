// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import {Test, console2} from "forge-std/Test.sol";
import "forge-std/console.sol";

import {MyToken} from "../MyToken.sol";
import "../searcher-vault/Structs.sol";

contract TestParsingHelpers is Test {
    struct AccountBalance {
        uint256 collateral;
        uint256 debt;
    }

    function keccakHash(
        string memory functionInterface
    ) public pure returns (bytes memory) {
        return abi.encodePacked(bytes4(keccak256(bytes(functionInterface))));
    }

    struct BidInfo {
        uint256 bid;
        uint256 deadline;
        address executor;
        uint256 executorSk;
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
        uint256 executorSk
    ) internal pure returns (BidInfo memory) {
        return BidInfo(bid, 1_000_000_000_000, vm.addr(executorSk), executorSk);
    }

    function assertAddressInArray(
        address addr,
        address[] memory arr,
        bool exists
    ) internal pure {
        bool found = false;
        for (uint256 i = 0; i < arr.length; i++) {
            if (arr[i] == addr) {
                found = true;
                break;
            }
        }
        assert(found == exists);
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

    function compareStrings(
        string memory a,
        string memory b
    ) public pure returns (bool) {
        return keccak256(abi.encodePacked(a)) == keccak256(abi.encodePacked(b));
    }
}

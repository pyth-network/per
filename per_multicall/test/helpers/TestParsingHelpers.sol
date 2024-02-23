// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {Test, console2} from "forge-std/Test.sol";
import "forge-std/console.sol";

import {MyToken} from "../../src/MyToken.sol";
import "../../src/Structs.sol";

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
        uint256 validUntil;
        address liquidator;
        uint256 liquidatorSk;
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

    function compareStrings(
        string memory a,
        string memory b
    ) public pure returns (bool) {
        return keccak256(abi.encodePacked(a)) == keccak256(abi.encodePacked(b));
    }
}

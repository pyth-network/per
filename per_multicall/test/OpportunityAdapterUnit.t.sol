// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import "forge-std/console.sol";
import "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import "../src/Errors.sol";
import "../src/Structs.sol";
import "../src/OpportunityAdapter.sol";
import "../src/OpportunityAdapterUpgradable.sol";
import "../src/MyToken.sol";
import "./helpers/Signatures/OpportunityAdapterSignature.sol";
import "./helpers/OpportunityAdapterHarness.sol";

contract OpportunityAdapterUnitTest is Test, OpportunityAdapterSignature {
    OpportunityAdapterHarness opportunityAdapter;
    MyToken myToken;

    function setUp() public {
        opportunityAdapter = new OpportunityAdapterHarness();
        myToken = new MyToken("SellToken", "ST");
    }

    function testPrepareSellTokens(uint256 tokenAmount) public {
        TokenAmount[] memory sellTokens = new TokenAmount[](1);
        sellTokens[0] = TokenAmount(address(myToken), tokenAmount);
        address executor = makeAddr("executor");
        myToken.mint(executor, tokenAmount);
        vm.prank(executor);
        myToken.approve(address(opportunityAdapter), tokenAmount);
        address targetContract = makeAddr("targetContract");
        opportunityAdapter.exposed_prepareSellTokens(
            sellTokens,
            executor,
            targetContract
        );
        assertEq(myToken.balanceOf(address(opportunityAdapter)), tokenAmount);
        assertEq(
            myToken.allowance(address(opportunityAdapter), targetContract),
            tokenAmount
        );
        assertEq(myToken.balanceOf(executor), 0);
    }

    function testCheckDuplicateTokens() public {
        TokenAmount[] memory tokens = new TokenAmount[](3);
        address token0 = makeAddr("token0");
        address token1 = makeAddr("token1");
        address token2 = makeAddr("token2");
        tokens[0] = TokenAmount(token0, 0);
        tokens[1] = TokenAmount(token1, 0);
        tokens[2] = TokenAmount(token2, 0);
        opportunityAdapter.exposed_checkDuplicateTokens(tokens);
        tokens[1] = TokenAmount(token2, 0);
        vm.expectRevert(DuplicateToken.selector);
        opportunityAdapter.exposed_checkDuplicateTokens(tokens);
    }

    function testGetContractTokenBalances(uint256 tokenAmount) public {
        TokenAmount[] memory tokens = new TokenAmount[](2);
        tokens[0] = TokenAmount(address(myToken), 0);
        tokens[1] = TokenAmount(address(myToken), 0);
        myToken.mint(address(opportunityAdapter), tokenAmount);
        uint256[] memory balances = opportunityAdapter
            .exposed_getContractTokenBalances(tokens);
        assertEq(balances[0], tokenAmount);
        assertEq(balances[1], tokenAmount);
    }

    function testRevertWhenTokenDoesNotExistInGetContractTokenBalances()
        public
    {
        TokenAmount[] memory tokens = new TokenAmount[](2);
        tokens[0] = TokenAmount(address(myToken), 0);
        tokens[1] = TokenAmount(makeAddr("InvalidToken"), 0);
        vm.expectRevert();
        uint256[] memory balances = opportunityAdapter
            .exposed_getContractTokenBalances(tokens);
    }

    function testValidateAndTransferBuyTokens(uint256 tokenAmount) public {
        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        buyTokens[0] = TokenAmount(address(myToken), tokenAmount);
        address executor = makeAddr("executor");
        address targetContract = makeAddr("targetContract");
        uint256[] memory buyTokensBalancesBeforeCall = new uint256[](1);
        buyTokensBalancesBeforeCall[0] = 0;
        myToken.mint(address(opportunityAdapter), tokenAmount);
        opportunityAdapter.exposed_validateAndTransferBuyTokens(
            buyTokens,
            executor,
            buyTokensBalancesBeforeCall
        );
        assertEq(myToken.balanceOf(address(opportunityAdapter)), 0);
        assertEq(myToken.balanceOf(executor), tokenAmount);
    }

    function testRevertWhenInsufficientTokensInValidateAndTransferBuyTokens(
        uint128 tokenAmount
    ) public {
        // tokenAmount is uint128 to avoid overflow in the test
        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        buyTokens[0] = TokenAmount(address(myToken), tokenAmount);
        address executor = makeAddr("executor");
        address targetContract = makeAddr("targetContract");
        uint256[] memory buyTokensBalancesBeforeCall = new uint256[](1);
        buyTokensBalancesBeforeCall[0] = 1; // not all tokens were received because of the call
        myToken.mint(address(opportunityAdapter), tokenAmount);
        vm.expectRevert(InsufficientTokenReceived.selector);
        opportunityAdapter.exposed_validateAndTransferBuyTokens(
            buyTokens,
            executor,
            buyTokensBalancesBeforeCall
        );
    }
}

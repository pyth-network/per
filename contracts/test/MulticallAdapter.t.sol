// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import "test/multicall-adapter/MulticallAdapter.sol";
import "test/multicall-adapter/Structs.sol";
import "./MyToken.sol";
import "./helpers/MulticallAdapterHarness.sol";

contract MockTarget {
    function doNothing() public {}
}

contract MulticallAdapterUnitTest is Test {
    MulticallAdapterHarness multicallAdapter;
    MockTarget mockTarget;
    MyToken sellToken1;
    MyToken sellToken2;

    function setUp() public {
        multicallAdapter = new MulticallAdapterHarness();
        mockTarget = new MockTarget();
        sellToken1 = new MyToken("SellToken1", "ST1");
        sellToken2 = new MyToken("SellToken2", "ST2");
    }

    function testTransferSellTokens(uint256 amount1, uint256 amount2) public {
        TokenAmount[] memory tokens = new TokenAmount[](2);
        tokens[0] = TokenAmount(address(sellToken1), amount1);
        tokens[1] = TokenAmount(address(sellToken2), amount2);
        address executor = makeAddr("executor");
        sellToken1.mint(executor, amount1);
        sellToken2.mint(executor, amount2);

        vm.startPrank(executor);

        sellToken1.approve(address(multicallAdapter), amount1);
        sellToken2.approve(address(multicallAdapter), amount2);
        multicallAdapter.exposed_transferSellTokens(tokens);

        vm.stopPrank();

        assertEq(sellToken1.balanceOf(address(multicallAdapter)), amount1);
        assertEq(sellToken2.balanceOf(address(multicallAdapter)), amount2);
        assertEq(sellToken1.balanceOf(executor), 0);
        assertEq(sellToken2.balanceOf(executor), 0);
    }

    function testApproveTokensRevokeAllowances(
        uint256 amount1,
        uint256 amount2
    ) public {
        TokenToSend[] memory tokensToSend = new TokenToSend[](2);
        tokensToSend[0] = TokenToSend({
            tokenAmount: TokenAmount(address(sellToken1), amount1),
            destination: makeAddr("target1")
        });
        tokensToSend[1] = TokenToSend({
            tokenAmount: TokenAmount(address(sellToken2), amount2),
            destination: makeAddr("target2")
        });

        multicallAdapter.exposed_approveTokens(tokensToSend);

        assertEq(
            sellToken1.allowance(
                address(multicallAdapter),
                makeAddr("target1")
            ),
            amount1
        );
        assertEq(
            sellToken2.allowance(
                address(multicallAdapter),
                makeAddr("target2")
            ),
            amount2
        );

        multicallAdapter.exposed_revokeAllowances(tokensToSend);

        assertEq(
            sellToken1.allowance(
                address(multicallAdapter),
                makeAddr("target1")
            ),
            0
        );
        assertEq(
            sellToken2.allowance(
                address(multicallAdapter),
                makeAddr("target2")
            ),
            0
        );
    }

    function testCallTargetContract() public {
        bytes memory targetCalldata = abi.encodeWithSelector(
            MockTarget.doNothing.selector
        );
        uint256 targetCallValue = 0;
        address targetContract = address(mockTarget);
        uint256 targetCallIndex = 0;

        vm.expectCall(targetContract, targetCallValue, targetCalldata);
        multicallAdapter.exposed_callTargetContract(
            targetContract,
            targetCalldata,
            targetCallValue,
            targetCallIndex
        );
    }

    function testSweepTokensTokenAmount(
        uint256 amount1,
        uint256 amount2
    ) public {
        TokenAmount[] memory tokens = new TokenAmount[](2);
        tokens[0] = TokenAmount(address(sellToken1), amount1);
        tokens[1] = TokenAmount(address(sellToken2), amount2);
        address executor = makeAddr("executor");
        sellToken1.mint(address(multicallAdapter), amount1);
        sellToken2.mint(address(multicallAdapter), amount2);

        vm.prank(executor);
        multicallAdapter.exposed_sweepTokensTokenAmount(tokens);

        assertEq(sellToken1.balanceOf(address(multicallAdapter)), 0);
        assertEq(sellToken2.balanceOf(address(multicallAdapter)), 0);
        assertEq(sellToken1.balanceOf(executor), amount1);
        assertEq(sellToken2.balanceOf(executor), amount2);
    }

    function testSweepTokensTokenToSend(
        uint256 amount1,
        uint256 amount2
    ) public {
        TokenToSend[] memory tokensToSend = new TokenToSend[](2);
        tokensToSend[0] = TokenToSend({
            tokenAmount: TokenAmount(address(sellToken1), amount1),
            destination: makeAddr("target1")
        });
        tokensToSend[1] = TokenToSend({
            tokenAmount: TokenAmount(address(sellToken2), amount2),
            destination: makeAddr("target2")
        });
        address executor = makeAddr("executor");
        sellToken1.mint(address(multicallAdapter), amount1);
        sellToken2.mint(address(multicallAdapter), amount2);

        vm.prank(executor);
        multicallAdapter.exposed_sweepTokensTokenToSend(tokensToSend);

        assertEq(sellToken1.balanceOf(address(multicallAdapter)), 0);
        assertEq(sellToken2.balanceOf(address(multicallAdapter)), 0);
        assertEq(sellToken1.balanceOf(executor), amount1);
        assertEq(sellToken2.balanceOf(executor), amount2);
    }
}

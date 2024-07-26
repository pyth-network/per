// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import "src/express-relay/Errors.sol";
import "src/opportunity-adapter/OpportunityAdapter.sol";
import {OpportunityAdapterFactory} from "src/opportunity-adapter/OpportunityAdapterFactory.sol";
import "./WETH9.sol";
import "./MyToken.sol";
import "./searcher-vault/Structs.sol";
import "permit2/interfaces/ISignatureTransfer.sol";
import {PermitSignature, EIP712Domain} from "./PermitSignature.sol";

contract MockTarget {
    address payable _weth;

    constructor(address weth) {
        _weth = payable(weth);
    }

    error BadCall();

    function doNothing() public payable {}

    function exchangeWethForWeth(
        uint256 amountIn,
        uint256 amountOut
    ) public payable {
        WETH9(_weth).transferFrom(msg.sender, address(this), amountIn);
        uint256 balanceWeth = WETH9(_weth).balanceOf(address(this));
        if (balanceWeth < amountOut) {
            WETH9(_weth).deposit{value: amountOut - balanceWeth}();
        }
        WETH9(_weth).transfer(msg.sender, amountOut);
    }

    function transferTokenToSender(
        address token,
        uint256 amount
    ) public payable {
        MyToken(token).mint(msg.sender, amount);
    }

    function transferSellTokenFromSenderAndBuyTokenToSender(
        address sellToken,
        uint256 sellAmount,
        address buyToken,
        uint256 buyAmount
    ) public payable {
        MyToken(sellToken).transferFrom(msg.sender, address(this), sellAmount);
        MyToken(buyToken).mint(msg.sender, buyAmount);
    }

    function revertCall() public payable {
        revert BadCall();
    }
}

contract OpportunityAdapterIntegrationTest is
    Test,
    PermitSignature,
    OpportunityAdapterHasher
{
    MockTarget mockTarget;
    OpportunityAdapterFactory adapterFactory;
    WETH9 weth;
    MyToken buyToken;
    MyToken sellToken;
    MyToken usdc;
    address _expressRelay;

    function setUpTokens() internal {
        buyToken = new MyToken("BuyToken", "BT");
        sellToken = new MyToken("SellToken", "ST");
        usdc = new MyToken("USDC", "USDC");
        weth = new WETH9();
    }

    function setUpOpportunityAdapter() internal {
        _expressRelay = makeAddr("expressRelay");
        adapterFactory = new OpportunityAdapterFactory(
            _expressRelay,
            address(weth),
            PermitSignature.PERMIT2
        );
    }

    function setUp() public {
        setUpTokens();
        setUpPermit2();
        setUpOpportunityAdapter();
        mockTarget = new MockTarget(address(weth));
    }

    // successful bids will be received by this contract
    receive() external payable {}

    function createSingularTargetCallMockTarget(
        TokenAmount[] memory sellTokens,
        bytes memory data,
        uint256 value
    ) public returns (TargetCall[] memory targetCalls) {
        targetCalls = new TargetCall[](1);
        TokenToSend[] memory tokensToSend = new TokenToSend[](
            sellTokens.length
        );
        for (uint j = 0; j < sellTokens.length; j++) {
            tokensToSend[j] = TokenToSend(sellTokens[j], address(mockTarget));
        }
        targetCalls[0] = TargetCall(
            address(mockTarget),
            data,
            value,
            tokensToSend
        );
    }

    function createExecutionParamsAndSignature(
        TokenAmount[] memory sellTokens,
        TokenAmount[] memory buyTokens,
        TargetCall[] memory targetCalls,
        uint256 bid,
        uint256 deadline
    )
        public
        returns (ExecutionParams memory executionParams, bytes memory signature)
    {
        ISignatureTransfer.TokenPermissions[]
            memory permitted = new ISignatureTransfer.TokenPermissions[](
                sellTokens.length
            );
        for (uint i = 0; i < sellTokens.length; i++) {
            permitted[i] = ISignatureTransfer.TokenPermissions(
                sellTokens[i].token,
                sellTokens[i].amount
            );
        }

        ISignatureTransfer.PermitBatchTransferFrom memory permit = ISignatureTransfer
            .PermitBatchTransferFrom(
                permitted,
                0, // TODO: fill in the nonce
                deadline
            );
        (address executor, uint256 executorSk) = makeAddrAndKey("executor");
        ExecutionWitness memory witness = ExecutionWitness(
            buyTokens,
            executor,
            targetCalls,
            bid
        );
        executionParams = ExecutionParams(permit, witness);
        signature = getPermitBatchWitnessSignature(
            permit,
            executorSk,
            FULL_OPPORTUNITY_WITNESS_BATCH_TYPEHASH,
            hash(witness),
            adapterFactory.computeAddress(executor),
            EIP712Domain(PERMIT2).DOMAIN_SEPARATOR()
        );
    }

    function createDummyExecutionParams(
        bool shouldRevert
    )
        internal
        returns (ExecutionParams memory executionParams, bytes memory signature)
    {
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        bytes memory targetCalldata;
        if (shouldRevert) {
            targetCalldata = abi.encodeWithSelector(
                mockTarget.revertCall.selector
            );
        } else {
            targetCalldata = abi.encodeWithSelector(
                mockTarget.doNothing.selector
            );
        }
        return
            createExecutionParamsAndSignature(
                noTokens,
                noTokens,
                createSingularTargetCallMockTarget(noTokens, targetCalldata, 0),
                0,
                block.timestamp + 1000
            );
    }

    function testRevertWhenInsufficientWethToTransferForCall() public {
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.doNothing.selector
        );
        uint callValue = 1;
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createExecutionParamsAndSignature(
                noTokens,
                noTokens,
                createSingularTargetCallMockTarget(
                    noTokens,
                    targetCalldata,
                    callValue
                ),
                0,
                block.timestamp + 1000
            );
        vm.prank(adapterFactory.getExpressRelay());
        // callvalue is 1 wei, but executor has not deposited/approved any WETH
        vm.expectRevert(InsufficientWethForTargetCallValue.selector);
        adapterFactory.executeOpportunity(executionParams, signature);
    }

    function testRevertWhenInsufficientWethToTransferForBid() public {
        address executor = makeAddr("executor");
        TokenAmount[] memory sellTokens = new TokenAmount[](1);
        uint256 callValue = 123;
        uint256 bid = 100;
        sellTokens[0] = TokenAmount(address(weth), callValue);
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.doNothing.selector
        );
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createExecutionParamsAndSignature(
                sellTokens,
                noTokens,
                createSingularTargetCallMockTarget(
                    sellTokens,
                    targetCalldata,
                    callValue
                ),
                bid,
                block.timestamp + 1000
            );
        vm.deal(executor, 1 ether);
        vm.startPrank(executor);
        weth.deposit{value: callValue}();
        weth.approve(PERMIT2, callValue);
        vm.stopPrank();
        vm.prank(adapterFactory.getExpressRelay());
        // callvalue is 123 wei, and executor has approved 123 wei so the call should succeed but adapter does not have
        // 100 more wei to return the bid
        vm.expectCall(address(mockTarget), callValue, targetCalldata);
        vm.expectRevert(InsufficientEthToSettleBid.selector);
        adapterFactory.executeOpportunity(executionParams, signature);
    }

    function testExecutionWithBidAndCallValue(
        uint256 buyTokenAmount,
        uint256 sellTokenAmount,
        uint256 targetCallValue,
        uint256 bidAmount
    ) public {
        vm.assume(bidAmount < type(uint256).max - targetCallValue);
        vm.assume(bidAmount > 0);
        vm.assume(targetCallValue > 0);
        vm.assume(sellTokenAmount > 0);
        vm.assume(buyTokenAmount < type(uint256).max - 5000);

        address executor = makeAddr("executor");
        TokenAmount[] memory sellTokens = new TokenAmount[](2);
        sellTokens[0] = TokenAmount(address(sellToken), sellTokenAmount);
        sellTokens[1] = TokenAmount(address(weth), targetCallValue + bidAmount);
        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        buyTokens[0] = TokenAmount(address(buyToken), buyTokenAmount);
        // transfer less sellToken than specified to test that allowances are revoked correctly
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.transferSellTokenFromSenderAndBuyTokenToSender.selector,
            address(sellToken),
            sellTokenAmount - 1,
            address(buyToken),
            buyTokenAmount
        );
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createExecutionParamsAndSignature(
                sellTokens,
                buyTokens,
                createSingularTargetCallMockTarget(
                    sellTokens,
                    targetCalldata,
                    targetCallValue
                ),
                bidAmount,
                block.timestamp + 1000
            );
        uint256 initialAdapterBuyTokenBalance = 5000;
        address opportunityAdapter = adapterFactory.computeAddress(executor);
        buyToken.mint(opportunityAdapter, initialAdapterBuyTokenBalance);
        sellToken.mint(executor, sellTokenAmount);
        vm.deal(executor, targetCallValue + bidAmount);
        vm.startPrank(executor);
        weth.deposit{value: (targetCallValue + bidAmount)}();
        weth.approve(PERMIT2, (targetCallValue + bidAmount));
        sellToken.approve(PERMIT2, sellTokenAmount);
        vm.stopPrank();
        vm.prank(adapterFactory.getExpressRelay());
        vm.expectCall(address(mockTarget), targetCallValue, targetCalldata);
        // We expect the adapter to transfer the bid to the express relay
        vm.expectCall(_expressRelay, bidAmount, bytes(""));
        vm.expectCall(
            address(weth),
            abi.encodeWithSelector(WETH9.withdraw.selector, targetCallValue)
        );
        vm.expectCall(
            address(weth),
            abi.encodeWithSelector(WETH9.withdraw.selector, bidAmount)
        );
        adapterFactory.executeOpportunity(executionParams, signature);
        assertEq(
            buyToken.balanceOf(executor),
            initialAdapterBuyTokenBalance + buyTokenAmount
        );
        assertEq(sellToken.balanceOf(executor), 0);
        assertEq(
            sellToken.allowance(opportunityAdapter, address(mockTarget)),
            0
        );
    }

    function testExecutionWithBidAndCallValueWithSwaps(
        uint256 buyTokenAmount,
        uint256 sellTokenAmount,
        uint256 targetCallValue,
        uint256 bidAmount,
        uint256 usdcSwapFromAmount,
        uint256 usdcSwapIntoAmount
    ) public {
        vm.assume(bidAmount < type(uint256).max - targetCallValue);
        vm.assume(bidAmount > 0);
        vm.assume(targetCallValue > 0);
        vm.assume(sellTokenAmount > 0);
        vm.assume(buyTokenAmount < type(uint256).max - 5000);
        vm.assume(usdcSwapFromAmount > 0);
        vm.assume(usdcSwapIntoAmount > 0);
        vm.assume(usdcSwapFromAmount < type(uint256).max - usdcSwapIntoAmount);

        address executor = makeAddr("executor");
        TokenAmount[] memory sellTokens = new TokenAmount[](2);
        sellTokens[0] = TokenAmount(address(usdc), usdcSwapFromAmount);
        sellTokens[1] = TokenAmount(address(weth), targetCallValue + bidAmount);
        TokenAmount[] memory sellTokensPostSwap = new TokenAmount[](2);
        sellTokensPostSwap[0] = TokenAmount(
            address(sellToken),
            sellTokenAmount
        );
        sellTokensPostSwap[1] = TokenAmount(
            address(weth),
            targetCallValue + bidAmount
        );

        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        buyTokens[0] = TokenAmount(address(usdc), usdcSwapIntoAmount);

        // transfer less sellToken than specified to test that allowances are revoked correctly
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.transferSellTokenFromSenderAndBuyTokenToSender.selector,
            address(sellToken),
            sellTokenAmount - 1,
            address(buyToken),
            buyTokenAmount
        );

        // create targetCalls with swaps
        TargetCall[]
            memory targetCallExecute = createSingularTargetCallMockTarget(
                sellTokensPostSwap,
                targetCalldata,
                targetCallValue
            );
        TokenAmount[] memory sellTokensSwapFrom = new TokenAmount[](1);
        sellTokensSwapFrom[0] = TokenAmount(address(usdc), usdcSwapFromAmount);
        TargetCall[]
            memory targetCallSwapFrom = createSingularTargetCallMockTarget(
                sellTokensSwapFrom,
                abi.encodeWithSelector(
                    mockTarget
                        .transferSellTokenFromSenderAndBuyTokenToSender
                        .selector,
                    address(usdc),
                    usdcSwapFromAmount,
                    address(sellToken),
                    sellTokenAmount
                ),
                0
            );
        TokenAmount[] memory sellTokensSwapInto = new TokenAmount[](1);
        sellTokensSwapInto[0] = TokenAmount(address(buyToken), buyTokenAmount);
        TargetCall[]
            memory targetCallSwapInto = createSingularTargetCallMockTarget(
                sellTokensSwapInto,
                abi.encodeWithSelector(
                    mockTarget
                        .transferSellTokenFromSenderAndBuyTokenToSender
                        .selector,
                    address(buyToken),
                    buyTokenAmount,
                    address(usdc),
                    usdcSwapIntoAmount
                ),
                0
            );

        TargetCall[] memory targetCalls = new TargetCall[](3);
        targetCalls[0] = targetCallSwapFrom[0];
        targetCalls[1] = targetCallExecute[0];
        targetCalls[2] = targetCallSwapInto[0];

        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createExecutionParamsAndSignature(
                sellTokens,
                buyTokens,
                targetCalls,
                bidAmount,
                block.timestamp + 1000
            );

        uint256 initialAdapterBuyTokenBalance = 5000;
        address opportunityAdapter = adapterFactory.computeAddress(executor);
        buyToken.mint(opportunityAdapter, initialAdapterBuyTokenBalance);
        usdc.mint(executor, usdcSwapFromAmount);
        vm.deal(executor, targetCallValue + bidAmount);
        vm.startPrank(executor);
        weth.deposit{value: (targetCallValue + bidAmount)}();
        weth.approve(PERMIT2, (targetCallValue + bidAmount));
        usdc.approve(PERMIT2, usdcSwapFromAmount);
        vm.stopPrank();
        vm.prank(adapterFactory.getExpressRelay());
        vm.expectCall(address(mockTarget), targetCallValue, targetCalldata);
        // We expect the adapter to transfer the bid to the express relay
        vm.expectCall(_expressRelay, bidAmount, bytes(""));
        vm.expectCall(
            address(weth),
            abi.encodeWithSelector(WETH9.withdraw.selector, targetCallValue)
        );
        vm.expectCall(
            address(weth),
            abi.encodeWithSelector(WETH9.withdraw.selector, bidAmount)
        );
        adapterFactory.executeOpportunity(executionParams, signature);
        assertEq(buyToken.balanceOf(executor), 0);
        assertEq(sellToken.balanceOf(executor), 0);
        assertEq(
            sellToken.allowance(opportunityAdapter, address(mockTarget)),
            0
        );

        assertEq(usdc.balanceOf(executor), usdcSwapIntoAmount);
        assertEq(usdc.allowance(opportunityAdapter, address(mockTarget)), 0);
    }

    function testExecutionWithNoBidAndCallValue() public {
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.doNothing.selector
        );
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createExecutionParamsAndSignature(
                noTokens,
                noTokens,
                createSingularTargetCallMockTarget(noTokens, targetCalldata, 0),
                0,
                block.timestamp + 1000
            );
        vm.prank(adapterFactory.getExpressRelay());
        vm.expectCall(address(mockTarget), targetCalldata);
        // When count is 0 (3rd argument) we expect 0 calls to be made to the specified address
        // In this case, we do not expect adapter to transfer any ETH to the express relay
        vm.expectCall(address(this), bytes(""), 0);
        vm.expectCall(
            address(weth),
            abi.encodeWithSelector(WETH9.withdraw.selector, 0),
            0
        );
        adapterFactory.executeOpportunity(executionParams, signature);
    }

    function testExecutionWithWethBuySellTokens(
        uint256 wethSellTokenAmount,
        uint256 wethBuyTokenAmount,
        uint256 bidAmount,
        uint256 targetCallValue
    ) public {
        vm.assume(bidAmount < type(uint256).max - targetCallValue);
        vm.assume(
            wethSellTokenAmount <
                type(uint256).max - bidAmount - targetCallValue
        );
        vm.assume(
            wethBuyTokenAmount <
                type(uint256).max -
                    wethSellTokenAmount -
                    bidAmount -
                    targetCallValue
        );

        TokenAmount[] memory sellTokens = new TokenAmount[](1);
        uint256 sellTokenAmount = wethSellTokenAmount +
            bidAmount +
            targetCallValue;
        sellTokens[0] = TokenAmount(address(weth), sellTokenAmount);

        TokenAmount[] memory buyTokens;
        if (wethBuyTokenAmount == 0) {
            buyTokens = new TokenAmount[](0);
        } else {
            buyTokens = new TokenAmount[](1);
            buyTokens[0] = TokenAmount(address(weth), wethBuyTokenAmount);
        }
        vm.deal(address(mockTarget), wethBuyTokenAmount);

        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.exchangeWethForWeth.selector,
            wethSellTokenAmount,
            wethBuyTokenAmount
        );
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createExecutionParamsAndSignature(
                sellTokens,
                buyTokens,
                createSingularTargetCallMockTarget(
                    sellTokens,
                    targetCalldata,
                    targetCallValue
                ),
                bidAmount,
                block.timestamp + 1000
            );

        vm.deal(executionParams.witness.executor, sellTokenAmount);
        vm.startPrank(executionParams.witness.executor);
        weth.deposit{value: sellTokenAmount}();
        weth.approve(PERMIT2, sellTokenAmount);
        vm.stopPrank();

        uint256 balanceWethPlusEthMockTargetPre = address(mockTarget).balance +
            WETH9(weth).balanceOf(address(mockTarget));

        vm.prank(adapterFactory.getExpressRelay());
        vm.expectCall(address(mockTarget), targetCalldata);
        adapterFactory.executeOpportunity(executionParams, signature);

        uint256 balanceWethPlusEthMockTargetPost = address(mockTarget).balance +
            WETH9(weth).balanceOf(address(mockTarget));

        assertEq(
            WETH9(weth).balanceOf(executionParams.witness.executor),
            wethBuyTokenAmount
        );
        assertEq(adapterFactory.getExpressRelay().balance, bidAmount);
        assertEq(
            balanceWethPlusEthMockTargetPost,
            balanceWethPlusEthMockTargetPre -
                wethBuyTokenAmount +
                targetCallValue +
                wethSellTokenAmount
        );
    }

    function testRevertWhenEthBalanceDecrease() public {
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.doNothing.selector
        );
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createExecutionParamsAndSignature(
                noTokens,
                noTokens,
                createSingularTargetCallMockTarget(noTokens, targetCalldata, 0),
                1,
                block.timestamp + 1000
            );
        vm.deal(adapterFactory.computeAddress(makeAddr("executor")), 1 ether);
        vm.prank(adapterFactory.getExpressRelay());
        vm.expectRevert(EthOrWethBalanceDecreased.selector);
        adapterFactory.executeOpportunity(executionParams, signature);
    }

    function testRevertWhenWethBalanceDecrease() public {
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.doNothing.selector
        );
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createExecutionParamsAndSignature(
                noTokens,
                noTokens,
                createSingularTargetCallMockTarget(noTokens, targetCalldata, 1),
                0,
                block.timestamp + 1000
            );
        vm.deal(adapterFactory.computeAddress(makeAddr("executor")), 1 ether);
        vm.prank(adapterFactory.computeAddress(makeAddr("executor")));
        weth.deposit{value: 1 ether}();
        vm.prank(adapterFactory.getExpressRelay());
        vm.expectRevert(EthOrWethBalanceDecreased.selector);
        adapterFactory.executeOpportunity(executionParams, signature);
    }

    function testRevertWhenInsufficientTokensReceived() public {
        TokenAmount[] memory sellTokens = new TokenAmount[](0);
        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        uint256 expectedBuyTokenAmount = 100;
        uint256 actualBuyTokenAmount = 99;
        buyTokens[0] = TokenAmount(address(buyToken), expectedBuyTokenAmount);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.transferTokenToSender.selector,
            address(buyToken),
            actualBuyTokenAmount
        );
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createExecutionParamsAndSignature(
                sellTokens,
                buyTokens,
                createSingularTargetCallMockTarget(
                    sellTokens,
                    targetCalldata,
                    0
                ),
                0,
                block.timestamp + 1000
            );
        buyToken.mint(address(adapterFactory), 1000); // initial balance should not affect the result
        vm.prank(adapterFactory.getExpressRelay());
        vm.expectCall(address(mockTarget), targetCalldata);
        vm.expectRevert(InsufficientTokenReceived.selector);
        adapterFactory.executeOpportunity(executionParams, signature);
    }

    function testRevertWhenNotCalledByExpressRelay() public {
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createDummyExecutionParams(false);
        vm.prank(makeAddr("nonRelayer"));
        vm.expectRevert(NotCalledByExpressRelay.selector);
        adapterFactory.executeOpportunity(executionParams, signature);
    }

    // TODO: we can remove this test since it is testing Permit2 and not our own contracts anymore
    function testRevertWhenSignatureReused() public {
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createDummyExecutionParams(false);
        vm.startPrank(adapterFactory.getExpressRelay());
        adapterFactory.executeOpportunity(executionParams, signature);
        vm.expectRevert(
            abi.encodeWithSelector(bytes4(keccak256("InvalidNonce()")))
        );
        adapterFactory.executeOpportunity(executionParams, signature);
        vm.stopPrank();
    }

    function testRevertWhenTargetCallFails() public {
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createDummyExecutionParams(true);
        vm.prank(adapterFactory.getExpressRelay());
        vm.expectRevert(
            abi.encodeWithSelector(
                TargetCallFailed.selector,
                abi.encodeWithSelector(MockTarget.BadCall.selector)
            )
        );
        adapterFactory.executeOpportunity(executionParams, signature);
    }

    function testGetWeth() public {
        assertEq(adapterFactory.getWeth(), address(weth));
    }

    function _testRevertWithDuplicateTokens(
        TokenAmount[] memory sellTokens,
        TokenAmount[] memory buyTokens
    ) public {
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.doNothing.selector
        );
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createExecutionParamsAndSignature(
                sellTokens,
                buyTokens,
                createSingularTargetCallMockTarget(
                    sellTokens,
                    targetCalldata,
                    0
                ),
                0,
                block.timestamp + 1000
            );
        vm.prank(adapterFactory.getExpressRelay());
        vm.expectRevert(DuplicateToken.selector);
        adapterFactory.executeOpportunity(executionParams, signature);
    }

    function testRevertWhenDuplicateBuyTokensOrDuplicateSellTokens() public {
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        TokenAmount[] memory duplicateTokens = new TokenAmount[](2);
        duplicateTokens[0] = TokenAmount(address(buyToken), 100);
        duplicateTokens[1] = TokenAmount(address(buyToken), 200);
        _testRevertWithDuplicateTokens(duplicateTokens, noTokens);
        _testRevertWithDuplicateTokens(noTokens, duplicateTokens);
    }

    // TODO: we can remove this test since it is testing Permit2 and not our own contracts anymore
    function testRevertWhenExpired() public {
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.doNothing.selector
        );
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createExecutionParamsAndSignature(
                noTokens,
                noTokens,
                createSingularTargetCallMockTarget(noTokens, targetCalldata, 0),
                0,
                block.timestamp + 1
            );
        vm.warp(block.timestamp + 2);
        vm.prank(adapterFactory.getExpressRelay());
        vm.expectRevert(
            abi.encodeWithSelector(
                bytes4(keccak256("SignatureExpired(uint256)")),
                executionParams.permit.deadline
            )
        );
        adapterFactory.executeOpportunity(executionParams, signature);
    }
}

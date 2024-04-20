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

contract MockTarget {
    error BadCall();

    function doNothing() public payable {}

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

contract OpportunityAdapterHarness is OpportunityAdapterUpgradable {
    function exposed_PrepareSellTokens(ExecutionParams calldata params) public {
        _prepareSellTokens(params);
    }
}

contract InvalidMagicOpportunityAdapter is
    Initializable,
    OwnableUpgradeable,
    UUPSUpgradeable
{
    function _authorizeUpgrade(address) internal override onlyOwner {}

    function opportunityAdapterUpgradableMagic() public pure returns (uint32) {
        return 0x00000000;
    }
}

contract OpportunityAdapterUnitTest is Test, OpportunityAdapterSignature {
    MockTarget mockTarget;
    OpportunityAdapterHarness opportunityAdapter;
    WETH9 weth;
    MyToken buyToken;
    MyToken sellToken;

    function setUp() public {
        buyToken = new MyToken("BuyToken", "BT");
        sellToken = new MyToken("SellToken", "ST");
        OpportunityAdapterHarness _opportunityAdapter = new OpportunityAdapterHarness();
        ERC1967Proxy proxyOpportunityAdapter = new ERC1967Proxy(
            address(_opportunityAdapter),
            ""
        );
        opportunityAdapter = OpportunityAdapterHarness(
            payable(proxyOpportunityAdapter)
        );
        weth = new WETH9();
        opportunityAdapter.initialize(
            address(this),
            address(this),
            address(this),
            address(weth)
        );
        mockTarget = new MockTarget();
    }

    function testRevertWhenInsufficientWethToTransferForCall() public {
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
                targetCalldata,
                1,
                0,
                block.timestamp + 1000
            );
        vm.prank(opportunityAdapter.getExpressRelay());
        // callvalue is 1 wei, but executor has not deposited/approved any WETH
        vm.expectRevert(WethTransferFromFailed.selector);
        opportunityAdapter.executeOpportunity(executionParams, signature);
    }

    function testRevertWhenInsufficientWethToTransferForBid() public {
        address executor = makeAddr("executor");
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
                targetCalldata,
                123,
                100,
                block.timestamp + 1000
            );
        vm.deal(executor, 1 ether);
        vm.startPrank(executor);
        weth.deposit{value: 123 wei}();
        weth.approve(address(opportunityAdapter), 123);
        vm.stopPrank();
        vm.prank(opportunityAdapter.getExpressRelay());
        // callvalue is 123 wei, and executor has approved 123 wei so the call should succeed but adapter does not have
        // 100 more wei to return the bid
        vm.expectCall(address(mockTarget), 123, targetCalldata);
        vm.expectRevert(WethTransferFromFailed.selector);
        opportunityAdapter.executeOpportunity(executionParams, signature);
    }

    // successful bids will be received by this contract
    receive() external payable {}

    function testExecutionWithBidAndCallValue() public {
        address executor = makeAddr("executor");
        TokenAmount[] memory sellTokens = new TokenAmount[](1);
        uint256 sellTokenAmount = 1000;
        sellTokens[0] = TokenAmount(address(sellToken), sellTokenAmount);
        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        uint256 buyTokenAmount = 100;
        buyTokens[0] = TokenAmount(address(buyToken), buyTokenAmount);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.transferSellTokenFromSenderAndBuyTokenToSender.selector,
            address(sellToken),
            sellTokenAmount,
            address(buyToken),
            buyTokenAmount
        );
        uint256 callValue = 123;
        uint256 bid = 100;
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createExecutionParamsAndSignature(
                sellTokens,
                buyTokens,
                targetCalldata,
                callValue,
                bid,
                block.timestamp + 1000
            );
        uint256 initialAdapterBuyTokenBalance = 5000;
        buyToken.mint(
            address(opportunityAdapter),
            initialAdapterBuyTokenBalance
        ); // initial balance should not affect the result
        sellToken.mint(executor, sellTokenAmount);
        vm.deal(executor, 1 ether);
        vm.startPrank(executor);
        weth.deposit{value: (callValue + bid)}();
        weth.approve(address(opportunityAdapter), (callValue + bid));
        sellToken.approve(address(opportunityAdapter), sellTokenAmount);
        vm.stopPrank();
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectCall(address(mockTarget), callValue, targetCalldata);
        vm.expectCall(address(this), bid, bytes(""));
        vm.expectCall(
            address(weth),
            abi.encodeWithSelector(WETH9.withdraw.selector, callValue)
        );
        vm.expectCall(
            address(weth),
            abi.encodeWithSelector(WETH9.withdraw.selector, bid)
        );
        opportunityAdapter.executeOpportunity(executionParams, signature);
        assertEq(buyToken.balanceOf(executor), buyTokenAmount);
        assertEq(
            buyToken.balanceOf(address(opportunityAdapter)),
            initialAdapterBuyTokenBalance
        );
        assertEq(sellToken.balanceOf(executor), 0);
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
                targetCalldata,
                0,
                0,
                block.timestamp + 1000
            );
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectCall(address(mockTarget), targetCalldata);
        // When count is 0 (3rd argument) we expect 0 calls to be made to the specified address
        vm.expectCall(address(this), bytes(""), 0);
        vm.expectCall(
            address(weth),
            abi.encodeWithSelector(WETH9.withdraw.selector, 0),
            0
        );
        opportunityAdapter.executeOpportunity(executionParams, signature);
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
                targetCalldata,
                0,
                0,
                block.timestamp + 1000
            );
        buyToken.mint(address(opportunityAdapter), 1000); // initial balance should not affect the result
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectCall(address(mockTarget), targetCalldata);
        vm.expectRevert(InsufficientTokenReceived.selector);
        opportunityAdapter.executeOpportunity(executionParams, signature);
    }

    function createExecutionParamsAndSignature(
        TokenAmount[] memory sellTokens,
        TokenAmount[] memory buyTokens,
        bytes memory data,
        uint256 value,
        uint256 bid,
        uint256 validUntil
    )
        public
        returns (ExecutionParams memory executionParams, bytes memory signature)
    {
        (address executor, uint256 executorSk) = makeAddrAndKey("executor");
        executionParams = ExecutionParams(
            sellTokens,
            buyTokens,
            executor,
            address(mockTarget),
            data,
            value,
            validUntil,
            bid
        );
        signature = createOpportunityAdapterSignature(
            opportunityAdapter,
            executionParams,
            executorSk
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
                targetCalldata,
                0,
                0,
                block.timestamp + 1000
            );
    }

    function testRevertWhenNotCalledByExpressRelay() public {
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createDummyExecutionParams(false);
        vm.prank(makeAddr("nonRelayer"));
        vm.expectRevert(Unauthorized.selector);
        opportunityAdapter.executeOpportunity(executionParams, signature);
    }

    function testRevertWhenSignatureReused() public {
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createDummyExecutionParams(false);
        vm.startPrank(opportunityAdapter.getExpressRelay());
        opportunityAdapter.executeOpportunity(executionParams, signature);
        vm.expectRevert(SignatureAlreadyUsed.selector);
        opportunityAdapter.executeOpportunity(executionParams, signature);
        vm.stopPrank();
    }

    function testRevertWhenTargetCallFails() public {
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createDummyExecutionParams(true);
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectRevert(
            abi.encodeWithSelector(
                TargetCallFailed.selector,
                abi.encodeWithSelector(MockTarget.BadCall.selector)
            )
        );
        opportunityAdapter.executeOpportunity(executionParams, signature);
    }

    function testGetWeth() public {
        assertEq(opportunityAdapter.getWeth(), address(weth));
    }

    function testSetExpressRelay() public {
        address newRelay = makeAddr("newRelay");
        opportunityAdapter.setExpressRelay(newRelay);
        assertEq(opportunityAdapter.getExpressRelay(), newRelay);
    }

    function testRevertWhenUnauthorizedSetExpressRelay() public {
        vm.prank(makeAddr("invalidAdmin"));
        vm.expectRevert(Unauthorized.selector);
        opportunityAdapter.setExpressRelay(address(this));
    }

    function testPrepareSellTokens() public {
        TokenAmount[] memory sellTokens = new TokenAmount[](1);
        TokenAmount[] memory buyTokens = new TokenAmount[](0);
        uint256 sellTokenAmount = 100;
        sellTokens[0] = TokenAmount(address(sellToken), sellTokenAmount);
        address executor = makeAddr("executor");
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createExecutionParamsAndSignature(
                sellTokens,
                buyTokens,
                bytes(""),
                0,
                0,
                block.timestamp + 1000
            );
        sellToken.mint(executor, sellTokenAmount);
        vm.prank(executor);
        sellToken.approve(address(opportunityAdapter), sellTokenAmount);
        opportunityAdapter.exposed_PrepareSellTokens(executionParams);
        assertEq(
            sellToken.balanceOf(address(opportunityAdapter)),
            sellTokenAmount
        );
        assertEq(
            sellToken.allowance(
                address(opportunityAdapter),
                address(mockTarget)
            ),
            sellTokenAmount
        );
        assertEq(buyToken.balanceOf(executor), 0);
    }

    function testRevertWhenDuplicateBuyTokensOrDuplicateSellTokens() public {
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        TokenAmount[] memory duplicateTokens = new TokenAmount[](2);
        duplicateTokens[0] = TokenAmount(address(buyToken), 100);
        duplicateTokens[1] = TokenAmount(address(buyToken), 200);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.doNothing.selector
        );
        (
            ExecutionParams memory executionParamsDuplicateBuy,
            bytes memory signatureBuy
        ) = createExecutionParamsAndSignature(
                noTokens,
                duplicateTokens,
                targetCalldata,
                0,
                0,
                block.timestamp + 1000
            );
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectRevert(DuplicateToken.selector);
        opportunityAdapter.executeOpportunity(
            executionParamsDuplicateBuy,
            signatureBuy
        );
        (
            ExecutionParams memory executionParamsDuplicateSell,
            bytes memory signatureSell
        ) = createExecutionParamsAndSignature(
                duplicateTokens,
                noTokens,
                targetCalldata,
                0,
                0,
                block.timestamp + 1000
            );
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectRevert(DuplicateToken.selector);
        opportunityAdapter.executeOpportunity(
            executionParamsDuplicateSell,
            signatureSell
        );
    }

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
                targetCalldata,
                0,
                0,
                block.timestamp + 1
            );
        vm.warp(block.timestamp + 2);
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectRevert(ExpiredSignature.selector);
        opportunityAdapter.executeOpportunity(executionParams, signature);
    }

    function testRevertWhenUpgradeWithWrongContract() public {
        vm.expectRevert("ERC1967Upgrade: new implementation is not UUPS");
        opportunityAdapter.upgradeTo(address(sellToken));
    }

    function testRevertWhenUpgradeWithWrongMagic() public {
        InvalidMagicOpportunityAdapter invalidMagicOpportunityAdapter = new InvalidMagicOpportunityAdapter();
        vm.prank(opportunityAdapter.owner());
        vm.expectRevert(InvalidMagicValue.selector);
        opportunityAdapter.upgradeTo(address(invalidMagicOpportunityAdapter));
    }

    function testSuccessfulUpgrade() public {
        OpportunityAdapterUpgradable newOpportunityAdapter = new OpportunityAdapterUpgradable();
        vm.prank(opportunityAdapter.owner());
        opportunityAdapter.upgradeTo(address(newOpportunityAdapter));
    }
}

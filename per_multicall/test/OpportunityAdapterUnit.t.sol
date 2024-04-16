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
import "./helpers/Signatures.sol";

contract MockTarget {
    error BadCall();

    function execute(bytes calldata data) public payable {}

    function executeAndReturnErc20(
        address token,
        uint256 amount
    ) public payable {
        MyToken(token).transfer(msg.sender, amount);
    }

    function executeGetAndReturnErc20(
        address sellToken,
        uint256 sellAmount,
        address buyToken,
        uint256 buyAmount
    ) public payable {
        MyToken(buyToken).transfer(msg.sender, buyAmount);
        MyToken(sellToken).transferFrom(msg.sender, address(this), sellAmount);
    }

    function revertCall() public payable {
        revert BadCall();
    }
}

contract OpportunityAdapterHarness is OpportunityAdapterUpgradable {
    function exposed_prepareSellTokens(ExecutionParams calldata params) public {
        _prepareSellTokens(params);
    }
}

contract OpportunityAdapterUnitTest is Test, Signatures {
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
        (, uint256 executorSk) = makeAddrAndKey("executor");
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.execute.selector,
            abi.encode(0)
        );
        ExecutionParams memory executionParams = createAndSignExecutionParams(
            noTokens,
            noTokens,
            address(mockTarget),
            targetCalldata,
            1,
            0,
            block.timestamp + 1000,
            executorSk
        );
        vm.prank(opportunityAdapter.getExpressRelay());
        // callvalue is 1 wei, but executor has not deposited/approved any WETH
        vm.expectRevert(WethTransferFromFailed.selector);
        opportunityAdapter.executeOpportunity(executionParams);
    }

    function testRevertWhenInsufficientWethToTransferForBid() public {
        (address executor, uint256 executorSk) = makeAddrAndKey("executor");
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.execute.selector,
            abi.encode("arbitrary data")
        );
        ExecutionParams memory executionParams = createAndSignExecutionParams(
            noTokens,
            noTokens,
            address(mockTarget),
            targetCalldata,
            123,
            100,
            block.timestamp + 1000,
            executorSk
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
        opportunityAdapter.executeOpportunity(executionParams);
    }

    // successful bids will be received by this contract
    receive() external payable {}

    function testExecutionWithBidAndCallValue() public {
        (address executor, uint256 executorSk) = makeAddrAndKey("executor");
        TokenAmount[] memory sellTokens = new TokenAmount[](1);
        sellTokens[0] = TokenAmount(address(sellToken), 1000);
        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        buyTokens[0] = TokenAmount(address(buyToken), 100);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.executeGetAndReturnErc20.selector,
            address(sellToken),
            1000,
            address(buyToken),
            100
        );
        ExecutionParams memory executionParams = createAndSignExecutionParams(
            sellTokens,
            buyTokens,
            address(mockTarget),
            targetCalldata,
            123,
            100,
            block.timestamp + 1000,
            executorSk
        );
        buyToken.mint(address(mockTarget), 100);
        sellToken.mint(executor, 1000);
        vm.deal(executor, 1 ether);
        vm.startPrank(executor);
        weth.deposit{value: 223 wei}();
        weth.approve(address(opportunityAdapter), 223);
        sellToken.approve(address(opportunityAdapter), 1000);
        vm.stopPrank();
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectCall(address(mockTarget), 123, targetCalldata);
        vm.expectCall(address(this), 100, bytes(""));
        vm.expectCall(
            address(weth),
            abi.encodeWithSelector(WETH9.withdraw.selector, 123)
        );
        vm.expectCall(
            address(weth),
            abi.encodeWithSelector(WETH9.withdraw.selector, 100)
        );
        opportunityAdapter.executeOpportunity(executionParams);
        assertEq(buyToken.balanceOf(executor), 100);
        assertEq(sellToken.balanceOf(executor), 0);
    }

    function testExecutionWithNoBidAndCallValue() public {
        (, uint256 executorSk) = makeAddrAndKey("executor");
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.execute.selector,
            abi.encode("arbitrary data")
        );
        ExecutionParams memory executionParams = createAndSignExecutionParams(
            noTokens,
            noTokens,
            address(mockTarget),
            targetCalldata,
            0,
            0,
            block.timestamp + 1000,
            executorSk
        );
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectCall(address(mockTarget), targetCalldata);
        // should not call the followings
        vm.expectCall(address(this), bytes(""), 0);
        vm.expectCall(
            address(weth),
            abi.encodeWithSelector(WETH9.withdraw.selector, 0),
            0
        );
        opportunityAdapter.executeOpportunity(executionParams);
    }

    function testRevertWhenInsufficientTokensReceived() public {
        (, uint256 executorSk) = makeAddrAndKey("executor");
        TokenAmount[] memory sellTokens = new TokenAmount[](0);
        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        buyTokens[0] = TokenAmount(address(buyToken), 100);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.execute.selector,
            abi.encode("arbitrary data")
        );
        ExecutionParams memory executionParams = createAndSignExecutionParams(
            sellTokens,
            buyTokens,
            address(mockTarget),
            targetCalldata,
            0,
            0,
            block.timestamp + 1000,
            executorSk
        );
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectCall(address(mockTarget), targetCalldata);
        vm.expectRevert(InsufficientTokenReceived.selector);
        opportunityAdapter.executeOpportunity(executionParams);
    }

    function createDummyExecutionParams(
        bool shouldRevert
    ) internal returns (ExecutionParams memory) {
        (, uint256 executorSk) = makeAddrAndKey("executor");
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        bytes memory targetCalldata;
        if (shouldRevert) {
            targetCalldata = abi.encodeWithSelector(
                mockTarget.revertCall.selector
            );
        } else {
            targetCalldata = abi.encodeWithSelector(
                mockTarget.execute.selector,
                abi.encode("arbitrary data")
            );
        }
        return
            createAndSignExecutionParams(
                noTokens,
                noTokens,
                address(mockTarget),
                targetCalldata,
                0,
                0,
                block.timestamp + 1000,
                executorSk
            );
    }

    function testRevertWhenNotCalledByExpressRelay() public {
        ExecutionParams memory executionParams = createDummyExecutionParams(
            false
        );
        (address invalidAdmin, ) = makeAddrAndKey("invalidExpressRelay");
        vm.prank(invalidAdmin);
        vm.expectRevert(Unauthorized.selector);
        opportunityAdapter.executeOpportunity(executionParams);
    }

    function testRevertWhenSignatureReused() public {
        ExecutionParams memory executionParams = createDummyExecutionParams(
            false
        );
        vm.startPrank(opportunityAdapter.getExpressRelay());
        opportunityAdapter.executeOpportunity(executionParams);
        vm.expectRevert(SignatureAlreadyUsed.selector);
        opportunityAdapter.executeOpportunity(executionParams);
        vm.stopPrank();
    }

    function testRevertWhenTargetCallFails() public {
        ExecutionParams memory executionParams = createDummyExecutionParams(
            true
        );
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectRevert(
            abi.encodeWithSelector(
                TargetCallFailed.selector,
                abi.encodeWithSelector(MockTarget.BadCall.selector)
            )
        );
        opportunityAdapter.executeOpportunity(executionParams);
    }

    function testGetWeth() public {
        assertEq(opportunityAdapter.getWeth(), address(weth));
    }

    function testSetExpressRelay() public {
        (address invalidAdmin, ) = makeAddrAndKey("invalidAdmin");
        vm.prank(invalidAdmin);
        vm.expectRevert(Unauthorized.selector);
        opportunityAdapter.setExpressRelay(address(this));

        (address newRelay, ) = makeAddrAndKey("newRelay");
        opportunityAdapter.setExpressRelay(newRelay);
        assertEq(opportunityAdapter.getExpressRelay(), newRelay);
    }

    function testPrepareSellTokens() public {
        TokenAmount[] memory sellTokens = new TokenAmount[](1);
        TokenAmount[] memory buyTokens = new TokenAmount[](0);
        sellTokens[0] = TokenAmount(address(buyToken), 100);
        (address executor, uint256 executorSk) = makeAddrAndKey("executor");
        ExecutionParams memory executionParams = createAndSignExecutionParams(
            sellTokens,
            buyTokens,
            address(mockTarget),
            bytes(""),
            1,
            0,
            block.timestamp + 1000,
            executorSk
        );
        buyToken.mint(executor, 100);
        vm.prank(executor);
        buyToken.approve(address(opportunityAdapter), 100);
        opportunityAdapter.exposed_prepareSellTokens(executionParams);
        assertEq(buyToken.balanceOf(address(opportunityAdapter)), 100);
        assertEq(
            buyToken.allowance(
                address(opportunityAdapter),
                address(mockTarget)
            ),
            100
        );
        assertEq(buyToken.balanceOf(executor), 0);
    }

    function testRevertWhenMultipleBuyTokensWithSameTokenNotFulfilled() public {
        (, uint256 executorSk) = makeAddrAndKey("executor");
        TokenAmount[] memory sellTokens = new TokenAmount[](0);
        TokenAmount[] memory buyTokens = new TokenAmount[](3);
        buyTokens[0] = TokenAmount(address(buyToken), 100);
        buyTokens[1] = TokenAmount(address(buyToken), 200);
        buyTokens[2] = TokenAmount(address(buyToken), 300);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.executeAndReturnErc20.selector,
            address(buyToken),
            300
        );
        ExecutionParams memory executionParams = createAndSignExecutionParams(
            sellTokens,
            buyTokens,
            address(mockTarget),
            targetCalldata,
            0,
            0,
            block.timestamp + 1000,
            executorSk
        );
        buyToken.mint(address(mockTarget), 300);
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectRevert(DuplicateToken.selector);
        opportunityAdapter.executeOpportunity(executionParams);
    }
}

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
    function exposedPrepareSellTokens(ExecutionParams calldata params) public {
        _prepareSellTokens(params);
    }
}

contract InvalidMagicOpportunityAdapter is
    Initializable,
    OwnableUpgradeable,
    UUPSUpgradeable
{
    // Only allow the owner to upgrade the proxy to a new implementation.
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
        (address executor, uint256 executorSk) = makeAddrAndKey("executor");
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.execute.selector,
            abi.encode(0)
        );
        ExecutionParams memory executionParams = createAndSignExecutionParams(
            opportunityAdapter,
            executor,
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
            opportunityAdapter,
            executor,
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
            opportunityAdapter,
            executor,
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
        uint256 initialAdapterBuyTokenBalance = 5000;
        buyToken.mint(
            address(opportunityAdapter),
            initialAdapterBuyTokenBalance
        ); // initial balance should not affect the result
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
        assertEq(
            buyToken.balanceOf(address(opportunityAdapter)),
            initialAdapterBuyTokenBalance
        );
        assertEq(sellToken.balanceOf(executor), 0);
    }

    function testExecutionWithNoBidAndCallValue() public {
        (address executor, uint256 executorSk) = makeAddrAndKey("executor");
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.execute.selector,
            abi.encode("arbitrary data")
        );
        ExecutionParams memory executionParams = createAndSignExecutionParams(
            opportunityAdapter,
            executor,
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
        (address executor, uint256 executorSk) = makeAddrAndKey("executor");
        TokenAmount[] memory sellTokens = new TokenAmount[](0);
        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        buyTokens[0] = TokenAmount(address(buyToken), 100);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.executeAndReturnErc20.selector,
            address(buyToken),
            99
        );
        ExecutionParams memory executionParams = createAndSignExecutionParams(
            opportunityAdapter,
            executor,
            sellTokens,
            buyTokens,
            address(mockTarget),
            targetCalldata,
            0,
            0,
            block.timestamp + 1000,
            executorSk
        );
        buyToken.mint(address(mockTarget), 100);
        buyToken.mint(address(opportunityAdapter), 1000); // initial balance should not affect the result
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectCall(address(mockTarget), targetCalldata);
        vm.expectRevert(InsufficientTokenReceived.selector);
        opportunityAdapter.executeOpportunity(executionParams);
    }

    function createDummyExecutionParams(
        bool shouldRevert
    ) internal returns (ExecutionParams memory) {
        (address executor, uint256 executorSk) = makeAddrAndKey("executor");
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
                opportunityAdapter,
                executor,
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
            opportunityAdapter,
            executor,
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
        opportunityAdapter.exposedPrepareSellTokens(executionParams);
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

    function testRevertWhenDuplicateTokens() public {
        (address executor, uint256 executorSk) = makeAddrAndKey("executor");
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        TokenAmount[] memory duplicateTokens = new TokenAmount[](2);
        duplicateTokens[0] = TokenAmount(address(buyToken), 100);
        duplicateTokens[1] = TokenAmount(address(buyToken), 200);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.execute.selector,
            abi.encode("arbitrary data")
        );
        ExecutionParams
            memory executionParamsDuplicateBuy = createAndSignExecutionParams(
                opportunityAdapter,
                executor,
                noTokens,
                duplicateTokens,
                address(mockTarget),
                targetCalldata,
                0,
                0,
                block.timestamp + 1000,
                executorSk
            );
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectRevert(DuplicateToken.selector);
        opportunityAdapter.executeOpportunity(executionParamsDuplicateBuy);
        ExecutionParams
            memory executionParamsDuplicateSell = createAndSignExecutionParams(
                opportunityAdapter,
                executor,
                duplicateTokens,
                noTokens,
                address(mockTarget),
                targetCalldata,
                0,
                0,
                block.timestamp + 1000,
                executorSk
            );
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectRevert(DuplicateToken.selector);
        opportunityAdapter.executeOpportunity(executionParamsDuplicateSell);
    }

    function testRevertWhenExpired() public {
        (address executor, uint256 executorSk) = makeAddrAndKey("executor");
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.execute.selector,
            abi.encode(0)
        );
        ExecutionParams memory executionParams = createAndSignExecutionParams(
            opportunityAdapter,
            executor,
            noTokens,
            noTokens,
            address(mockTarget),
            targetCalldata,
            0,
            0,
            block.timestamp + 1,
            executorSk
        );
        vm.warp(block.timestamp + 2);
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectRevert(ExpiredSignature.selector);
        opportunityAdapter.executeOpportunity(executionParams);
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

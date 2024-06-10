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
import "permit2/interfaces/ISignatureTransfer.sol";
import {PermitSignature, EIP712Domain} from "./PermitSignature.sol";

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

contract OpportunityAdapterIntegrationTest is
    Test,
    OpportunityAdapterSignature,
    PermitSignature
{
    MockTarget mockTarget;
    OpportunityAdapterUpgradable opportunityAdapter;
    WETH9 weth;
    MyToken buyToken;
    MyToken sellToken;

    function setUpTokens() internal {
        buyToken = new MyToken("BuyToken", "BT");
        sellToken = new MyToken("SellToken", "ST");
        weth = new WETH9();
    }

    function setUpOpportunityAdapter() internal {
        OpportunityAdapterUpgradable _opportunityAdapter = new OpportunityAdapterUpgradable();
        ERC1967Proxy proxyOpportunityAdapter = new ERC1967Proxy(
            address(_opportunityAdapter),
            ""
        );
        opportunityAdapter = OpportunityAdapterUpgradable(
            payable(proxyOpportunityAdapter)
        );
        opportunityAdapter.initialize(
            address(this),
            address(this),
            address(this),
            address(weth)
        );
    }

    function setUp() public {
        setUpTokens();
        setUpPermit2();
        setUpOpportunityAdapter();
        mockTarget = new MockTarget();
    }

    // successful bids will be received by this contract
    receive() external payable {}

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
                validUntil
            );
        (address executor, uint256 executorSk) = makeAddrAndKey("executor");
        OpportunityWitness memory witness = OpportunityWitness(
            buyTokens,
            executor,
            address(mockTarget),
            data,
            value,
            bid
        );
        executionParams = ExecutionParams(permit, witness);
        signature = getPermitBatchWitnessSignature(
            permit,
            executorSk,
            FULL_WITNESS_BATCH_TYPEHASH,
            opportunityAdapter.hash(witness),
            address(opportunityAdapter),
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
                targetCalldata,
                0,
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
                targetCalldata,
                callValue,
                0,
                block.timestamp + 1000
            );
        vm.prank(opportunityAdapter.getExpressRelay());
        // callvalue is 1 wei, but executor has not deposited/approved any WETH
        vm.expectRevert(InsufficientWethForTargetCallValue.selector);
        opportunityAdapter.executeOpportunity(executionParams, signature);
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
                targetCalldata,
                callValue,
                bid,
                block.timestamp + 1000
            );
        vm.deal(executor, 1 ether);
        vm.startPrank(executor);
        weth.deposit{value: callValue}();
        weth.approve(PERMIT2, callValue);
        vm.stopPrank();
        vm.prank(opportunityAdapter.getExpressRelay());
        // callvalue is 123 wei, and executor has approved 123 wei so the call should succeed but adapter does not have
        // 100 more wei to return the bid
        vm.expectCall(address(mockTarget), callValue, targetCalldata);
        vm.expectRevert(InsufficientEthToSettleBid.selector);
        opportunityAdapter.executeOpportunity(executionParams, signature);
    }

    function testExecutionWithBidAndCallValue() public {
        address executor = makeAddr("executor");
        TokenAmount[] memory sellTokens = new TokenAmount[](2);
        uint256 sellTokenAmount = 1000;
        uint256 callValue = 123;
        uint256 bid = 100;
        sellTokens[0] = TokenAmount(address(sellToken), sellTokenAmount);
        sellTokens[1] = TokenAmount(address(weth), callValue + bid);
        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        uint256 buyTokenAmount = 100;
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
        weth.approve(PERMIT2, (callValue + bid));
        sellToken.approve(PERMIT2, sellTokenAmount);
        vm.stopPrank();
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectCall(address(mockTarget), callValue, targetCalldata);
        // We expect the adapter to transfer the bid to the express relay
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
        assertEq(
            sellToken.allowance(
                address(opportunityAdapter),
                address(mockTarget)
            ),
            0
        );
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
        // In this case, we do not expect adapter to transfer any ETH to the express relay
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

    function testRevertWhenNotCalledByExpressRelay() public {
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createDummyExecutionParams(false);
        vm.prank(makeAddr("nonRelayer"));
        vm.expectRevert(Unauthorized.selector);
        opportunityAdapter.executeOpportunity(executionParams, signature);
    }

    // TODO: we can remove this test since it is testing Permit2 and not our own contracts anymore
    function testRevertWhenSignatureReused() public {
        (
            ExecutionParams memory executionParams,
            bytes memory signature
        ) = createDummyExecutionParams(false);
        vm.startPrank(opportunityAdapter.getExpressRelay());
        opportunityAdapter.executeOpportunity(executionParams, signature);
        vm.expectRevert(
            abi.encodeWithSelector(bytes4(keccak256("InvalidNonce()")))
        );
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
                targetCalldata,
                0,
                0,
                block.timestamp + 1000
            );
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectRevert(DuplicateToken.selector);
        opportunityAdapter.executeOpportunity(executionParams, signature);
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
                targetCalldata,
                0,
                0,
                block.timestamp + 1
            );
        vm.warp(block.timestamp + 2);
        vm.prank(opportunityAdapter.getExpressRelay());
        vm.expectRevert(
            abi.encodeWithSelector(
                bytes4(keccak256("SignatureExpired(uint256)")),
                executionParams.permit.deadline
            )
        );
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

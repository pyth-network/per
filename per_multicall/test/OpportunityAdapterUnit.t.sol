// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import "forge-std/console.sol";
import "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import "../src/Errors.sol";
import "../src/Structs.sol";
import "../src/OpportunityAdapter.sol";
import "../src/OpportunityAdapterUpgradable.sol";
import "./helpers/Signatures/OpportunityAdapterSignature.sol";

contract MockTarget {
    function execute(bytes calldata data) public payable {}
}

contract OpportunityAdapterUnitTest is Test, OpportunityAdapterSignature {
    MockTarget mockTarget;
    OpportunityAdapterUpgradable opportunityAdapter;
    WETH9 weth;

    function setUp() public {
        OpportunityAdapterUpgradable _opportunityAdapter = new OpportunityAdapterUpgradable();
        ERC1967Proxy proxyOpportunityAdapter = new ERC1967Proxy(
            address(_opportunityAdapter),
            ""
        );
        opportunityAdapter = OpportunityAdapterUpgradable(
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
        ExecutionParams memory executionParams = ExecutionParams(
            noTokens,
            noTokens,
            executor,
            address(mockTarget),
            targetCalldata,
            0,
            block.timestamp + 1000,
            100
        );
        bytes memory signature = createSignature(
            opportunityAdapter,
            executionParams,
            executorSk
        );
        vm.prank(opportunityAdapter.getExpressRelay());
        // callvalue is 1 wei, but executor has not deposited/approved any WETH
        vm.expectRevert(WethTransferFromFailed.selector);
        opportunityAdapter.executeOpportunity(executionParams, signature);
    }

    function testRevertWhenInsufficientWethToTransferForBid() public {
        (address executor, uint256 executorSk) = makeAddrAndKey("executor");
        TokenAmount[] memory noTokens = new TokenAmount[](0);
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.execute.selector,
            abi.encode("arbitrary data")
        );

        ExecutionParams memory executionParams = ExecutionParams(
            noTokens,
            noTokens,
            executor,
            address(mockTarget),
            targetCalldata,
            123,
            block.timestamp + 1000,
            100
        );
        bytes memory signature = createSignature(
            opportunityAdapter,
            executionParams,
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
        opportunityAdapter.executeOpportunity(executionParams, signature);
    }
}

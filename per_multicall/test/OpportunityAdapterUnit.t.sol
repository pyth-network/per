// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import "forge-std/console.sol";

import "../src/Errors.sol";
import "../src/Structs.sol";
import "../src/OpportunityAdapter.sol";
import "../src/OpportunityAdapterUpgradable.sol";
import "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import "./helpers/Signatures.sol";

contract MockTarget {
    function execute(bytes calldata data) public payable {}
}

contract OpportunityAdapterUnitTest is Test, Signatures {
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

    function test_RevertWhen_InsufficientWethToTransferForCall() public {
        (address executor, uint256 executorSk) = makeAddrAndKey("executor");
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
        vm.startPrank(opportunityAdapter.owner());
        vm.expectRevert(WethTransferFromFailed.selector);
        opportunityAdapter.executeOpportunity(executionParams);
        vm.stopPrank();
    }

    function test_RevertWhen_InsufficientWethToTransferForBid() public {
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
        vm.startPrank(opportunityAdapter.owner());
        vm.expectCall(address(mockTarget), 123, targetCalldata);
        vm.expectRevert(WethTransferFromFailed.selector);
        opportunityAdapter.executeOpportunity(executionParams);
        vm.stopPrank();
    }
}

// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";

import "src/opportunity-adapter/OpportunityAdapterFactory.sol";
import "./helpers/OpportunityAdapterFactoryHarness.sol";

contract OpportunityAdapterFactoryUnitTest is
    Test,
    IOpportunityAdapterFactory,
    OpportunityAdapterHasher
{
    struct Parameters {
        address expressRelay;
        address weth;
        address permit2;
        address owner;
    }

    Parameters public override parameters;

    address _expressRelay;
    address _weth;
    address _permit2;

    OpportunityAdapterFactoryHarness opportunityAdapterFactory;

    error Create2FailedDeployment();

    function setUp() public {
        _expressRelay = makeAddr("expressRelay");
        _weth = makeAddr("weth");
        _permit2 = makeAddr("permit2");
        opportunityAdapterFactory = new OpportunityAdapterFactoryHarness(
            _expressRelay,
            _weth,
            _permit2
        );
    }

    function testIsContract() public {
        assert(opportunityAdapterFactory.exposed_isContract(address(this)));

        assert(!opportunityAdapterFactory.exposed_isContract(address(0)));
        assert(
            !opportunityAdapterFactory.exposed_isContract(address(0xdeadbeef))
        );
    }

    function testComputeAddress(address owner) public {
        address adapter = opportunityAdapterFactory.createAdapter(owner);

        assertEq(adapter, opportunityAdapterFactory.computeAddress(owner));
    }

    function testCreateAdapter(address owner) public {
        verifyParams0(opportunityAdapterFactory);

        address adapter = opportunityAdapterFactory.createAdapter(owner);

        verifyParams0(opportunityAdapterFactory);

        OpportunityAdapter opportunityAdapterOwner = OpportunityAdapter(
            payable(adapter)
        );
        assertEq(opportunityAdapterOwner.getExpressRelay(), _expressRelay);
        assertEq(opportunityAdapterOwner.getWeth(), _weth);
    }

    function testRevertCreateDuplicateAdapter(address owner) public {
        opportunityAdapterFactory.createAdapter(owner);
        vm.expectRevert(Create2FailedDeployment.selector);
        opportunityAdapterFactory.createAdapter(owner);
    }

    function testRevertExecuteOpportunity() public {
        address targetContract = _permit2;

        ExecutionWitness memory witness = ExecutionWitness({
            buyTokens: new TokenAmount[](0),
            targetCalldata: new bytes(0),
            targetCallValue: 0,
            targetContract: targetContract,
            executor: makeAddr("executor"),
            bidAmount: 0
        });

        ISignatureTransfer.TokenPermissions[]
            memory permitted = new ISignatureTransfer.TokenPermissions[](0);
        ISignatureTransfer.PermitBatchTransferFrom
            memory permit = ISignatureTransfer.PermitBatchTransferFrom({
                permitted: permitted,
                nonce: 0,
                deadline: 0
            });

        ExecutionParams memory params = ExecutionParams({
            permit: permit,
            witness: witness
        });

        bytes memory signature;

        address opportunityAdapterExecutor = opportunityAdapterFactory
            .computeAddress(witness.executor);
        bytes memory expectedData = abi.encodeCall(
            OpportunityAdapter.executeOpportunity,
            (params, signature)
        );

        vm.prank(_expressRelay);
        vm.expectCall(opportunityAdapterExecutor, expectedData);
        vm.expectRevert(TargetContractNotAllowed.selector);
        opportunityAdapterFactory.executeOpportunity(params, signature);
    }

    function verifyParams0(
        OpportunityAdapterFactoryHarness opportunityAdapterFactory
    ) internal {
        (
            address expressRelay,
            address weth,
            address permit2,
            address owner
        ) = IOpportunityAdapterFactory(address(opportunityAdapterFactory))
                .parameters();

        assertEq(expressRelay, address(0));
        assertEq(weth, address(0));
        assertEq(permit2, address(0));
        assertEq(owner, address(0));
    }
}

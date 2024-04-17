// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Signature.sol";
import "../../../src/OpportunityAdapterUpgradable.sol";

contract OpportunityAdapterSignature is Signature {
    bytes constant _TYPE_HASH =
        "Signature(ExecutionParams executionParams,address signer,uint256 deadline)ExecutionParams(TokenAmount[] sellTokens,TokenAmount[] buyTokens,address targetContract,bytes targetCalldata,uint256 targetCallValue,uint256 bidAmount)TokenAmount(address token,uint256 amount)";
    string constant _NAME = "OpportunityAdapter";
    string constant _VERSION = "1";

    function initializeOpportunityAdapterSignature() public initializer {
        _initializeSignature(_NAME, _VERSION);
    }

    function createAndSignExecutionParams(
        OpportunityAdapterUpgradable opportunityAdapter,
        address signer,
        TokenAmount[] memory sellTokens,
        TokenAmount[] memory buyTokens,
        address target,
        bytes memory data,
        uint256 value,
        uint256 bid,
        uint256 validUntil,
        uint256 executorSk
    ) public view returns (ExecutionParams memory executionParams) {
        ExecutionParams memory fakeExecutionParams = ExecutionParams(
            sellTokens,
            buyTokens,
            vm.addr(executorSk),
            target,
            data,
            value,
            validUntil,
            bid,
            abi.encodePacked("0", "0", "0")
        );
        bytes32 digest = _hashTypedDataV4(
            address(opportunityAdapter),
            _NAME,
            _VERSION,
            _TYPE_HASH,
            signer,
            opportunityAdapter.hashExecutionParams(fakeExecutionParams),
            validUntil
        );
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(executorSk, digest);
        executionParams = ExecutionParams(
            sellTokens,
            buyTokens,
            vm.addr(executorSk),
            target,
            data,
            value,
            validUntil,
            bid,
            abi.encodePacked(r, s, v)
        );
        return executionParams;
    }
}

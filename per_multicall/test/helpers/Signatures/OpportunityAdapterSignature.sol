// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Signature.sol";

contract OpportunityAdapterSignature is Signature {
    bytes constant _TYPE_HASH =
        "Opportunity(TokenAmount sellTokens,TokenAmount buyTokens,address targetContract,bytes targetCalldata,uint256 targetCallValue,uint256 bidAmount,uint256 validUntil)TokenAmount(address token,uint256 amount)";
    string constant _NAME = "OpportunityAdapter";
    string constant _VERSION = "1";

    function initializeOpportunityAdapterSignature() public initializer {
        _initializeSignature(_NAME, _VERSION);
    }

    function createAndSignExecutionParams(
        address contractAddress,
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
        bytes32 digest = _hashTypedDataV4(
            contractAddress,
            _NAME,
            _VERSION,
            _TYPE_HASH,
            signer,
            abi.encode(
                sellTokens,
                buyTokens,
                target,
                data,
                value,
                bid,
                validUntil
            ),
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

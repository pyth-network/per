// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Signature.sol";
import "../../../src/OpportunityAdapterUpgradable.sol";

contract OpportunityAdapterSignature is Signature {
    string constant _NAME = "OpportunityAdapter";
    string constant _VERSION = "1";

    function initializeOpportunityAdapterSignature() public initializer {
        _initializeSignature(_NAME, _VERSION);
    }

    function createSignature(
        OpportunityAdapterUpgradable opportunityAdapter,
        ExecutionParams memory executionParams,
        uint256 executorSk
    ) public view returns (bytes memory) {
        bytes32 hashedData = opportunityAdapter.hash(executionParams);
        bytes32 domainSeparator = _domainSeparatorV4(
            address(opportunityAdapter),
            _NAME,
            _VERSION
        );
        return createSignature(hashedData, domainSeparator, executorSk);
    }
}

// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Signature.sol";

contract SearcherSignature is Signature {
    bytes constant _TYPE_HASH =
        "ExecutionParams(uint256 vaultId,uint256 bid, address signer, uint256 validUntil)";
    string constant _NAME = "Searcher";
    string constant _VERSION = "1";

    function initializeSearcherSignature() public initializer {
        _initializeSignature(_NAME, _VERSION);
    }

    function createSignature(
        address contractAddress,
        address signer,
        uint256 dataNumber,
        uint256 bid,
        uint256 validUntil,
        uint256 searcherSk
    ) public view returns (bytes memory) {
        bytes32 domainSeparator = _domainSeparatorV4(
            contractAddress,
            _NAME,
            _VERSION
        );
        bytes32 hashedData = keccak256(
            abi.encode(
                keccak256(_TYPE_HASH),
                dataNumber,
                bid,
                signer,
                validUntil
            )
        );
        return createSignature(hashedData, domainSeparator, searcherSk);
    }
}

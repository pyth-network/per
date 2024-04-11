// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Signature.sol";

contract SearcherSignature is Signature {
    bytes constant _TYPE_HASH =
        "Liquidation(uint256 vaultId,uint256 bid,uint256 validUntil)";
    string constant _NAME = "Searcher";
    string constant _VERSION = "1";

    function initializeSearcherSignature() public initializer {
        _initializeSignature(_NAME, _VERSION);
    }

    function createSearcherSignature(
        address contractAddress,
        address signer,
        uint256 dataNumber,
        uint256 bid,
        uint256 validUntil,
        uint256 searcherSk
    ) public view returns (bytes memory) {
        bytes32 digest = _hashTypedDataV4(
            contractAddress,
            _NAME,
            _VERSION,
            _TYPE_HASH,
            signer,
            abi.encode(dataNumber, bid, validUntil),
            validUntil
        );
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(searcherSk, digest);
        return abi.encodePacked(r, s, v);
    }
}

// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "openzeppelin-contracts/contracts/utils/cryptography/ECDSA.sol";
import "openzeppelin-contracts/contracts/utils/cryptography/EIP712.sol";

import "./Errors.sol";

contract SigVerify is EIP712 {
    mapping(bytes => bool) _signatureUsed;

    // Signatures from different versions are not compatible.
    constructor(
        string memory name,
        string memory version
    ) EIP712(name, version) {}

    function verifyCalldata(
        bytes memory typeHash,
        address _signer,
        bytes memory _data,
        bytes memory signature,
        uint256 deadline
    ) internal view {
        bytes32 digest = _hashTypedDataV4(
            keccak256(abi.encode(keccak256(typeHash), _signer, _data, deadline))
        );
        address signer = ECDSA.recover(digest, signature);

        if (signer == address(0) || signer != _signer) {
            revert InvalidExecutorSignature();
        }

        if (block.timestamp > deadline) {
            revert ExpiredSignature();
        }

        if (_signatureUsed[signature] == true) {
            revert SignatureAlreadyUsed();
        }
    }

    function _useSignature(bytes memory signature) internal {
        _signatureUsed[signature] = true;
    }
}

// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "openzeppelin-contracts/contracts/utils/cryptography/ECDSA.sol";
import "openzeppelin-contracts-upgradeable/contracts/utils/cryptography/EIP712Upgradeable.sol";

import "./Errors.sol";
import "forge-std/console.sol";

contract SigVerify is EIP712Upgradeable {
    mapping(bytes => bool) _signatureUsed;

    function verifyCalldata(
        bytes memory rawType,
        address _signer,
        bytes memory _data,
        bytes memory signature,
        uint256 deadline
    ) public view {
        bytes32 digest = _hashTypedDataV4(
            keccak256(abi.encode(keccak256(rawType), _signer, _data, deadline))
        );
        address signer = ECDSA.recover(digest, signature);

        if (signer == address(0) || signer != _signer) {
            revert InvalidSignature();
        }

        if (block.timestamp > deadline) {
            revert ExpiredSignature();
        }

        if (_signatureUsed[signature]) {
            revert SignatureAlreadyUsed();
        }
    }

    function _useSignature(bytes memory signature) internal {
        _signatureUsed[signature] = true;
    }
}

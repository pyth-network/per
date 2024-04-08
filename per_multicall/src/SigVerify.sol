// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "openzeppelin-contracts/contracts/utils/cryptography/ECDSA.sol";
import "openzeppelin-contracts/contracts/utils/cryptography/EIP712.sol";

import "./Errors.sol";

contract SigVerify is EIP712 {
    mapping(bytes => bool) _signatureUsed;
    mapping(address => uint256) _nonces;

    // Signatures from different versions are not compatible.
    constructor(
        string memory name,
        string memory version
    ) EIP712(name, version) {}

    function _verifyCalldata(
        address _signer,
        bytes memory _data,
        bytes memory signature,
        uint256 deadline
    ) internal {
        bytes32 digest = _hashTypedDataV4(
            keccak256(
                abi.encode(
                    keccak256(
                        "_verifyCalldata(address _signer, bytes memory _data, uint256 nonce, uint256 deadline)"
                    ),
                    _signer,
                    _data,
                    _nonces[_signer],
                    deadline
                )
            )
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

        _nonces[_signer]++;
    }

    function _useSignature(bytes memory signature) internal {
        _signatureUsed[signature] = true;
    }
}

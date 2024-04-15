// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "openzeppelin-contracts/contracts/utils/cryptography/ECDSA.sol";
import "openzeppelin-contracts-upgradeable/contracts/utils/cryptography/EIP712Upgradeable.sol";

import "./Errors.sol";
import "forge-std/console.sol";

contract SigVerify is EIP712Upgradeable {
    mapping(bytes => bool) _signatureUsed;
    string constant _SIGNATURE_TYPE =
        "Signature(ExecutionParams executionParams,address signer,uint256 deadline)";

    function verifyCalldata(
        string memory executionParamsType,
        bytes32 hashed_data,
        address signer,
        bytes memory signature,
        uint256 deadline
    ) public view {
        bytes32 digest = _hashTypedDataV4(
            keccak256(
                abi.encode(
                    keccak256(
                        bytes(
                            string.concat(_SIGNATURE_TYPE, executionParamsType)
                        )
                    ),
                    hashed_data,
                    signer,
                    deadline
                )
            )
        );
        address _signer = ECDSA.recover(digest, signature);

        if (_signer == address(0) || _signer != signer) {
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

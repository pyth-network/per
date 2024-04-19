// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "openzeppelin-contracts/contracts/utils/cryptography/ECDSA.sol";
import "openzeppelin-contracts-upgradeable/contracts/utils/cryptography/EIP712Upgradeable.sol";

import "./Errors.sol";
import "forge-std/console.sol";

contract SigVerify is EIP712Upgradeable {
    mapping(bytes => bool) _signatureUsed;
    string constant _SIGNATURE_TYPE =
        "SignedParams(ExecutionParams executionParams,address signer,uint256 deadline)";

    /**
     * @notice Verifies the validity of the provided calldata signature and parameters.
     * @dev This function computes the eip712 data hash from the provided parameters and compares it
     * with the ecdsa recovered signer from the provided signature. It also checks for the
     * validity of the signature, expiration of the deadline, and whether the signature
     * has already been used.
     * @param executionParamsType The type of execution parameters. This is used to create the complete typed data hash by concatenating it with the signature type.
     * @param hashedData The eip712 hashed data constructed from the execution parameters with the provided type.
     * @param signer The expected signer address.
     * @param signature The signature to be verified.
     * @param deadline The deadline timestamp in seconds until which the signature is valid.
     * @dev Throws `InvalidSignature` if the signature is invalid or doesn't match the signer.
     * Throws `ExpiredSignature` if the signature has expired based on the provided deadline.
     * Throws `SignatureAlreadyUsed` if the signature has already been used.
     */
    function verifyCalldata(
        string memory executionParamsType,
        bytes32 hashedData,
        address signer,
        bytes memory signature,
        uint256 deadline
    ) public view {
        if (block.timestamp > deadline) {
            revert ExpiredSignature();
        }

        if (_signatureUsed[signature]) {
            revert SignatureAlreadyUsed();
        }

        bytes32 digest = _hashTypedDataV4(
            keccak256(
                abi.encode(
                    keccak256(
                        bytes(
                            string.concat(_SIGNATURE_TYPE, executionParamsType)
                        )
                    ),
                    hashedData,
                    signer,
                    deadline
                )
            )
        );
        address _signer = ECDSA.recover(digest, signature);

        if (_signer == address(0) || _signer != signer) {
            revert InvalidSignature();
        }
    }

    function _useSignature(bytes memory signature) internal {
        _signatureUsed[signature] = true;
    }
}

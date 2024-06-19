// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import "openzeppelin-contracts/contracts/utils/cryptography/ECDSA.sol";
import "openzeppelin-contracts-upgradeable/contracts/utils/cryptography/EIP712Upgradeable.sol";

import "./Errors.sol";

contract SigVerify is EIP712Upgradeable {
    mapping(bytes => bool) _signatureUsed;

    /**
     * @notice Verifies the validity of the provided calldata signature and parameters.
     * @dev This function computes the eip712 data hash from the provided parameters and compares it
     * with the ecdsa recovered signer from the provided signature. It also checks for the
     * validity of the signature, expiration timestamp, and whether the signature
     * has already been used.
     * @param hashedData The eip712 hashed data constructed for signature verification.
     * @param signer The expected signer address.
     * @param signature The signature to be verified.
     * @param validUntil The latest timestamp in seconds until which the signature is valid.
     * @dev Throws `InvalidSignature` if the signature is invalid or doesn't match the signer.
     * Throws `ExpiredSignature` if the signature has expired based on the provided validUntil.
     * Throws `SignatureAlreadyUsed` if the signature has already been used.
     */
    function verifyCalldata(
        bytes32 hashedData,
        address signer,
        bytes memory signature,
        uint256 validUntil
    ) public view {
        if (block.timestamp > validUntil) {
            revert ExpiredSignature();
        }

        if (_signatureUsed[signature]) {
            revert SignatureAlreadyUsed();
        }

        bytes32 digest = _hashTypedDataV4(hashedData);
        address _signer = ECDSA.recover(digest, signature);

        if (_signer == address(0) || _signer != signer) {
            revert InvalidSignature();
        }
    }

    function _useSignature(bytes memory signature) internal {
        _signatureUsed[signature] = true;
    }
}

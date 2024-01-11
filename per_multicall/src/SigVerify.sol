// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./Errors.sol";
import "forge-std/console.sol";

contract SigVerify {
    function getMessageDigest(
        string memory _message,
        uint _nonce
    ) public pure returns (bytes32) {
        return keccak256(abi.encodePacked(_message, _nonce));
    }

    function getPERSignedMessageDigest(
        bytes32 _messageHash
    ) public pure returns (bytes32) {
        /*
        Signature is produced by signing a keccak256 hash with the following format:
        "\x19PER Signed Message\n" + msg
        */
        return
            keccak256(
                abi.encodePacked("\x19PER Signed Message:\n66", _messageHash)
            );
    }

    function getCalldataDigest(
        bytes memory _data,
        uint _nonce
    ) public pure returns (bytes32) {
        return keccak256(abi.encodePacked(_data, _nonce));
    }

    function verifyCalldata(
        address _signer,
        bytes memory _data,
        uint _nonce,
        bytes memory signature
    ) public pure returns (bool) {
        bytes32 calldataHash = getCalldataDigest(_data, _nonce);

        return recoverSigner(calldataHash, signature) == _signer;
    }

    function recoverSigner(
        bytes32 _ethSignedMessageHash,
        bytes memory _signature
    ) public pure returns (address) {
        (bytes32 r, bytes32 s, uint8 v) = splitSignature(_signature);

        return ecrecover(_ethSignedMessageHash, v, r, s);
    }

    function splitSignature(
        bytes memory sig
    ) public pure returns (bytes32 r, bytes32 s, uint8 v) {
        require(sig.length == 65, "invalid signature length");

        assembly {
            /*
            First 32 bytes stores the length of the signature

            add(sig, 32) = pointer of sig + 32
            effectively, skips first 32 bytes of signature

            mload(p) loads next 32 bytes starting at the memory address p into memory
            */

            // first 32 bytes, after the length prefix
            r := mload(add(sig, 32))
            // second 32 bytes
            s := mload(add(sig, 64))
            // final byte (first byte of the next 32 bytes)
            v := byte(0, mload(add(sig, 96)))
        }

        // implicitly return (r, s, v)
    }
}

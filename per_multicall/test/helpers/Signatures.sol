// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "../../src/Structs.sol";
import "../../src/SigVerify.sol";
import {Test} from "forge-std/Test.sol";
import "forge-std/console.sol";

import "openzeppelin-contracts/contracts/utils/Strings.sol";
import "openzeppelin-contracts/contracts/utils/cryptography/ECDSA.sol";
import "openzeppelin-contracts/contracts/utils/cryptography/MessageHashUtils.sol";

contract Signatures is Test, SigVerify {
    function _domainSeparatorV4(
        address contractAddress
    ) private view returns (bytes32) {
        return
            keccak256(
                abi.encode(
                    keccak256(
                        "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
                    ),
                    keccak256(bytes("SigVerify")),
                    keccak256(bytes("1")),
                    block.chainid,
                    contractAddress
                )
            );
    }

    function _hashTypedDataV4(
        address contractAddress,
        address signer,
        bytes memory data,
        uint256 deadline
    ) internal view virtual returns (bytes32) {
        return
            MessageHashUtils.toTypedDataHash(
                _domainSeparatorV4(contractAddress),
                keccak256(
                    abi.encode(
                        EIP712_SIGN_TYPEHASH,
                        signer,
                        data,
                        _nonces[signer],
                        deadline
                    )
                )
            );
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
            signer,
            abi.encode(dataNumber, bid, validUntil),
            validUntil
        );
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(searcherSk, digest);
        return abi.encodePacked(r, s, v);
    }

    function createAndSignExecutionParams(
        address contractAddress,
        address signer,
        TokenAmount[] memory sellTokens,
        TokenAmount[] memory buyTokens,
        address target,
        bytes memory data,
        uint256 value,
        uint256 bid,
        uint256 validUntil,
        uint256 executorSk
    ) public view returns (ExecutionParams memory executionParams) {
        bytes32 digest = _hashTypedDataV4(
            contractAddress,
            signer,
            abi.encode(
                sellTokens,
                buyTokens,
                target,
                data,
                value,
                bid,
                validUntil
            ),
            validUntil
        );
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(executorSk, digest);
        executionParams = ExecutionParams(
            sellTokens,
            buyTokens,
            vm.addr(executorSk),
            target,
            data,
            value,
            validUntil,
            bid,
            abi.encodePacked(r, s, v)
        );
        return executionParams;
    }
}

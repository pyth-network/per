// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "../../../src/Structs.sol";
import "../../../src/SigVerify.sol";
import {Test} from "forge-std/Test.sol";
import "forge-std/console.sol";

import "openzeppelin-contracts/contracts/utils/Strings.sol";
import "openzeppelin-contracts/contracts/utils/cryptography/ECDSA.sol";
import "openzeppelin-contracts/contracts/utils/cryptography/MessageHashUtils.sol";

contract Signature is Test, SigVerify {
    function _initializeSignature(
        string memory domainName,
        string memory domainVersion
    ) internal {
        __EIP712_init(domainName, domainVersion);
    }

    function _domainSeparatorV4(
        address contractAddress,
        string memory name,
        string memory version
    ) public view returns (bytes32) {
        return
            keccak256(
                abi.encode(
                    keccak256(
                        "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
                    ),
                    keccak256(bytes(name)),
                    keccak256(bytes(version)),
                    block.chainid,
                    contractAddress
                )
            );
    }

    function createSignature(
        bytes32 hashedData,
        bytes32 domainSeparator,
        uint256 signerSk
    ) public pure returns (bytes memory) {
        bytes32 digest = MessageHashUtils.toTypedDataHash(
            domainSeparator,
            hashedData
        );
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(signerSk, digest);
        return abi.encodePacked(r, s, v);
    }
}

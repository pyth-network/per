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

    function _customDomainSeparatorV4(
        address contractAddress,
        string memory name,
        string memory version
    ) private view returns (bytes32) {
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

    function _customHashTypedDataV4(
        address contractAddress,
        string memory name,
        string memory version,
        bytes memory typeHash,
        address signer,
        bytes32 hashedData,
        uint256 deadline
    ) internal view virtual returns (bytes32) {
        return
            MessageHashUtils.toTypedDataHash(
                _customDomainSeparatorV4(contractAddress, name, version),
                keccak256(
                    abi.encode(
                        keccak256(typeHash),
                        hashedData,
                        signer,
                        deadline
                    )
                )
            );
    }
}

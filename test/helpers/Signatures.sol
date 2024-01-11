// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "../../src/SigVerify.sol";
import {Test} from "forge-std/Test.sol";

import "openzeppelin-contracts/contracts/utils/Strings.sol";

contract Signatures is Test, SigVerify {
    function createSearcherSignature(
        uint256 dataNumber,
        uint256 bid,
        uint256 blockNumber,
        uint256 searcherSk
    ) public pure returns (bytes memory) {
        bytes memory dataSearcher = abi.encodePacked(dataNumber, bid);
        bytes32 calldataHash = getCalldataDigest(dataSearcher, blockNumber);
        (uint8 vSearcher, bytes32 rSearcher, bytes32 sSearcher) = vm.sign(
            searcherSk,
            calldataHash
        );
        return abi.encodePacked(rSearcher, sSearcher, vSearcher);
    }

    function createPerSignature(
        uint256 signaturePerVersionNumber,
        address protocolAddress,
        uint256 blockNumber,
        uint256 perOperatorSk
    ) public pure returns (bytes memory) {
        string memory messagePer = Strings.toHexString(
            uint160(protocolAddress),
            20
        );
        bytes32 messageDigestPer = getMessageDigest(messagePer, blockNumber);
        bytes32 signedMessageDigestPer = getPERSignedMessageDigest(
            messageDigestPer
        );
        (uint8 vPer, bytes32 rPer, bytes32 sPer) = vm.sign(
            perOperatorSk,
            signedMessageDigestPer
        );
        return abi.encodePacked(signaturePerVersionNumber, rPer, sPer, vPer);
    }
}

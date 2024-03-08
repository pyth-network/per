// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "../../src/Structs.sol";
import "../../src/SigVerify.sol";
import {Test} from "forge-std/Test.sol";

import "openzeppelin-contracts/contracts/utils/Strings.sol";

contract Signatures is Test, SigVerify {
    function createSearcherSignature(
        uint256 dataNumber,
        uint256 bid,
        uint256 validUntil,
        uint256 searcherSk
    ) public pure returns (bytes memory) {
        bytes32 calldataHash = keccak256(
            abi.encode(dataNumber, bid, validUntil)
        );
        (uint8 vSearcher, bytes32 rSearcher, bytes32 sSearcher) = vm.sign(
            searcherSk,
            calldataHash
        );
        return abi.encodePacked(rSearcher, sSearcher, vSearcher);
    }

    function createLiquidationSignature(
        TokenAmount[] memory repayTokens,
        TokenAmount[] memory expectedReceiptTokens,
        address contractAddress,
        bytes memory data,
        uint256 value,
        uint256 bid,
        uint256 validUntil,
        uint256 liquidatorSk
    ) public pure returns (bytes memory) {
        bytes32 calldataDigestLiquidator = keccak256(
            abi.encode(
                repayTokens,
                expectedReceiptTokens,
                contractAddress,
                data,
                value,
                bid,
                validUntil
            )
        );
        (uint8 vLiquidator, bytes32 rLiquidator, bytes32 sLiquidator) = vm.sign(
            liquidatorSk,
            calldataDigestLiquidator
        );
        return abi.encodePacked(rLiquidator, sLiquidator, vLiquidator);
    }
}

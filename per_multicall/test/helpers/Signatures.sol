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
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(searcherSk, calldataHash);
        return abi.encodePacked(r, s, v);
    }

    function createAndSignExecutionParams(
        TokenAmount[] memory sellTokens,
        TokenAmount[] memory buyTokens,
        address target,
        bytes memory data,
        uint256 value,
        uint256 bid,
        uint256 validUntil,
        uint256 executorSk
    ) public pure returns (ExecutionParams memory executionParams) {
        bytes32 calldataDigestExecutor = keccak256(
            abi.encode(
                sellTokens,
                buyTokens,
                target,
                data,
                value,
                bid,
                validUntil
            )
        );
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(
            executorSk,
            calldataDigestExecutor
        );
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

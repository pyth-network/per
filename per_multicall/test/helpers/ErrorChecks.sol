// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

contract ErrorChecks {
    function keccakHash(
        string memory functionInterface
    ) public pure returns (bytes memory) {
        return abi.encodePacked(bytes4(keccak256(bytes(functionInterface))));
    }
}

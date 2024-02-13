// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "solidity-bytes-utils/contracts/BytesLib.sol";

contract ErrorChecks {
    function keccakHash(
        string memory functionInterface
    ) public pure returns (bytes memory) {
        bytes memory hashOutput = abi.encodePacked(
            keccak256(abi.encodePacked(functionInterface))
        );
        return BytesLib.slice(hashOutput, 0, 4);
    }
}

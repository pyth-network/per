// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

contract ExpressRelayHelpers {
    function isContract(address addr) internal view returns (bool) {
        uint32 size;
        assembly {
            size := extcodesize(addr)
        }
        return (size > 0);
    }

    function bytesToAddress(
        bytes memory bys
    ) internal pure returns (address addr) {
        // extract the first 20 bytes and convert to an address
        addr = address(uint160(uint256(bytes32(bys))));
    }
}

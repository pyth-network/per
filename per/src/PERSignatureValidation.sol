// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./Errors.sol";
import "forge-std/console.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";
import "./PERRegistry.sol";
import "./Structs.sol";

contract PERSignatureValidation {
    function validateSignaturePER(bytes calldata signaturePER, address protocol, address perOperator) public view returns (bool validSignature) {
        revert NotImplemented();

        /// This method is called by the protocol to check that a call to it via PER was authorized. 
        /// In particular, it checks that the PER operator authorized this call by attaching a 
        /// valid signature of the protocol address and the current block number.
    }
}

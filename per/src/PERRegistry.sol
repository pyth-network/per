// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./Errors.sol";
import "forge-std/console.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";
import "./Structs.sol";

contract PERRegistry {
    mapping(address => FeeMetadata) _registry;
    uint256 _defaultFeeSplitProtocol;
    uint256 _defaultFeeSplitPrecision;

    /**
     * @notice Registry constructor - Initializes a new registry contract with given default fee parameters
     * 
     * @param defaultFeeSplitProtocol: the default fee percentage that will be diverted to registered protocols
     * @param defaultFeeSplitPrecision: the default fee split precision
     */
    constructor(
        uint256 defaultFeeSplitProtocol,
        uint256 defaultFeeSplitPrecision
    ) {
        _defaultFeeSplitProtocol = defaultFeeSplitProtocol;
        _defaultFeeSplitPrecision = defaultFeeSplitPrecision;

        require(_defaultFeeSplitProtocol <= _defaultFeeSplitPrecision, "invalid fee split");
    }

    /**
     * @notice registerContract function - registers the contract that calls this method for fee split
     * Note that the contract in question must call directly into registerContract
     */
    function registerContract() public {
        _registry[msg.sender] = FeeMetadata(_defaultFeeSplitProtocol, _defaultFeeSplitPrecision);
    }

    /**
     * @notice deregisterContract function - deregisters the contract that calls this method for fee split
     * Note that the contract in question must call directly into deregisterContract
     */
    function deregisterContract() public {
        delete _registry[msg.sender];
    }

    /**
     * @notice getFeeMetadata function - returns the fee metadata for a given contract address
     * 
     * @param contractAddress: address of the contract to get fee metadata for
     */
    function getFeeMetadata(address contractAddress) public view returns (FeeMetadata memory) {
        return _registry[contractAddress];
    }
}

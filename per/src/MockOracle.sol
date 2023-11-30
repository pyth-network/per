// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./Errors.sol";
import "./Structs.sol";

contract MockOracle {
    // coin --> (delay --> state)
    mapping(address => mapping(uint256 => OracleState)) _state;

    function setPrice(address token, uint256 timestamp, uint256 price) public {
        revert NotImplemented();

        /// This method is called to set the price of a given token at a given timestamp. It saves
        /// the price and timestamp in an OracleState struct within a mapping. It determines the
        /// delay to save these values at by subtracting the timestamp from the current block 
        /// timestamp. This is a simplified way to determine the delay, but it is sufficient for
        /// the purposes of this mock oracle. Note that this method will revert if the timestamp
        /// passed in exceeds the current block timestamp.
    }

    function getPrice(address token, uint256 delay) external view returns (OracleState memory) {
        return _state[token][delay];
    }
}

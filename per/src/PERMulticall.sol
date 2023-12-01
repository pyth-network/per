// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./Errors.sol";
import "forge-std/console.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";

contract PERMulticall {
    event ReceivedETH(address sender, uint256 amount);

    address _perOperator;
    address _perRegistry;
    address _perSignatureValidation;

    /**
     * @notice PERMulticall constructor - Initializes a new multicall contract with given parameters
     * 
     * @param perOperatorAddress: address of PER operator EOA
     * @param perRegistryAddress: address of PER registry contract
     * @param perSignatureValidationAddress: address of PER signature validation contract
     */
    constructor(
        address perOperatorAddress,
        address perRegistryAddress,
        address perSignatureValidationAddress
    ) {
        _perOperator = perOperatorAddress;
        _perRegistry = perRegistryAddress;
        _perSignatureValidation = perSignatureValidationAddress;
    }

    /**
     * @notice setPEROperator function - sets the address of the PER operator
     * 
     * @param perOperatorAddress: address of PER operator EOA
     */
    function setPEROperator(address perOperatorAddress) public {
        revert NotImplemented();
    }

    /**
     * @notice setPERRegistry function - sets the address of the PER registry
     * 
     * @param perRegistryAddress: address of PER registry contract
     */
    function setPERRegistry(address perRegistryAddress) public {
        revert NotImplemented();
    }

    /**
     * @notice setPERSignatureValidation function - sets the address of the PER Signature Validation contract
     * 
     * @param perSignatureValidationAddress: address of PER signature validation contract
     */
    function setPERSignatureValidation(address perSignatureValidationAddress) public {
        revert NotImplemented();
    }

    /**
     * @notice getPEROperator function - returns the address of the PER operator
     */
    function getPEROperator() public view returns (address) {
        return _perOperator;
    }

    /**
     * @notice getPERRegistry function - returns the address of the PER registry
     */
    function getPERRegistry() public view returns (address) {
        return _perRegistry;
    }

    /**
     * @notice getPERSignatureValidation function - returns the address of the PER signature validation contract
     */
    function getPERSignatureValidation() public view returns (address) {
        return _perSignatureValidation;
    }
    


    /**
     * @notice multicall function - performs a number of calls to external contracts in order
     * 
     * @param contracts: ordered list of contracts to call into
     * @param data: ordered list of calldata to call with
     * @param bids: ordered list of bids; call i will fail if it does not pay PER operator at least bid i
     * @param protocols: ordered list of protocols (address) whose fast functions are being called
     */
    function multicall(
		address[] calldata contracts,
        bytes[] calldata data,
        uint256[] calldata bids,
        address[] calldata protocols
    ) public payable returns (bytes[] memory externalResults, string[] memory multicallRevertReasons) {
        revert NotImplemented();
        
        /// This method will be the point of entry for all interactions with the multicall contract.
        /// The PER operator will call into this method with a bundle of searcher calls.
        /// This method will run through each call in the bundle and call into the callWithBid method
        /// via a try-catch. This way, if a call to callWithBid fails, it will return the revert 
        /// reason to this method, and it will not fail the entire parent call made to multicall.
    }

    /**
     * @notice callWithBid function - contained call to function with check for bid invariant
     * 
     * @param contractAddress: contract address to call into
     * @param data: calldata to call with
     * @param bid: bid to be paid; call will fail if it does not pay PER operator at least bid,
     * @param protocol: protocol whose fast function is being called 
     */
    function callWithBid(
        address contractAddress,
        bytes calldata data,
        uint256 bid,
        address protocol
    ) public payable returns (bytes memory) {        
        revert NotImplemented();

        /// This method will be called by the multicall method. It will call into the provided
        /// contractAddress and perform the call with the provided calldata. It will then check
        /// whether the call was successful. If it was not, it will return the revert reason.
        /// If the call was successful, it will check whether the PER operator was paid at least
        /// the specified bid. If it was not, it will revert. If it was, it will transfer the
        /// appropriate split of the bid to the protocol.
    }

    receive() external payable  { 
        emit ReceivedETH(msg.sender, msg.value);
    }
}

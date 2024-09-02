// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import "./OpportunityAdapter.sol";
import "./Structs.sol";
import "openzeppelin-contracts/contracts/utils/ReentrancyGuard.sol";
import "openzeppelin-contracts/contracts/utils/Create2.sol";
import {IOpportunityAdapterFactory} from "./IOpportunityAdapterFactory.sol";

contract OpportunityAdapterFactory is
    ReentrancyGuard,
    IOpportunityAdapterFactory
{
    address _expressRelay;
    address _weth;
    address _permit2;
    bytes32 _codeHash;

    struct Parameters {
        address expressRelay;
        address weth;
        address permit2;
        address owner;
    }

    Parameters public override parameters;

    /**
     * @notice OpportunityAdapterFactory constructor
     *
     * @param expressRelay: address of express relay
     * @param weth: address of WETH contract
     * @param permit2: address of permit2 contract
     */
    constructor(address expressRelay, address weth, address permit2) {
        _expressRelay = expressRelay;
        _weth = weth;
        _permit2 = permit2;
        _codeHash = keccak256(type(OpportunityAdapter).creationCode);
    }

    function createAdapter(address owner) public returns (address) {
        bytes32 salt = bytes32(uint256(uint160(owner)));
        parameters = Parameters({
            expressRelay: _expressRelay,
            weth: _weth,
            permit2: _permit2,
            owner: owner
        });
        address adapter = Create2.deploy(
            0,
            salt,
            type(OpportunityAdapter).creationCode
        );
        delete parameters;
        return adapter;
    }

    function _isContract(address addr) internal view returns (bool) {
        uint32 size;
        assembly {
            size := extcodesize(addr)
        }
        return (size > 0);
    }

    function computeAddress(address owner) public view returns (address) {
        bytes32 salt = bytes32(uint256(uint160(owner)));
        return Create2.computeAddress(salt, _codeHash);
    }

    /**
     * @notice getWeth function - returns the address of the WETH contract used for wrapping and unwrapping ETH
     */
    function getWeth() public view returns (address) {
        return _weth;
    }

    /**
     * @notice getPermit2 function - returns the address of the permit2 contract used for token approvals
     */
    function getPermit2() public view returns (address) {
        return _permit2;
    }

    /**
     * @notice getOpportunityAdapterCreationCodeHash - returns the hash of the creation code of the opportunity adapter contract
     */
    function getOpportunityAdapterCreationCodeHash()
        public
        view
        returns (bytes32)
    {
        return _codeHash;
    }

    /**
     * @notice getExpressRelay function - returns the address of the express relay authenticated for calling this contract
     */
    function getExpressRelay() public view returns (address) {
        return _expressRelay;
    }

    function executeOpportunity(
        ExecutionParams calldata params,
        bytes calldata signature
    ) public payable nonReentrant {
        if (msg.sender != _expressRelay) {
            revert NotCalledByExpressRelay();
        }
        address adapterAddress = computeAddress(params.witness.executor);
        if (!_isContract(adapterAddress)) {
            createAdapter(params.witness.executor);
        }
        // msg.value should be 0 but we will pass it to the adapter anyway in case it changes in the future
        OpportunityAdapter(payable(adapterAddress)).executeOpportunity{
            value: msg.value
        }(params, signature);
    }
}

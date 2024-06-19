// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import "./OpportunityAdapter.sol";
import "./Structs.sol";
import "openzeppelin-contracts/contracts/utils/ReentrancyGuard.sol";
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
        OpportunityAdapter adapter = new OpportunityAdapter{salt: salt}();
        delete parameters;
        return address(adapter);
    }

    function isContract(address addr) internal view returns (bool) {
        uint32 size;
        assembly {
            size := extcodesize(addr)
        }
        return (size > 0);
    }

    function computeAddress(address owner) public view returns (address) {
        uint8 prefix = 0xff;
        bytes32 salt = bytes32(uint256(uint160(owner)));
        bytes32 hash = keccak256(
            abi.encodePacked(prefix, address(this), salt, _codeHash)
        );
        return address(uint160(uint256(hash)));
    }

    /**
     * @notice getWeth function - returns the address of the WETH contract used for wrapping and unwrapping ETH
     */
    function getWeth() public view returns (address) {
        return _weth;
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
        if (!isContract(adapterAddress)) {
            createAdapter(params.witness.executor);
        }
        // msg.value should be 0 but we will pass it to the adapter anyway in case it changes in the future
        OpportunityAdapter(payable(adapterAddress)).executeOpportunity{
            value: msg.value
        }(params, signature);
    }
}

// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./Errors.sol";
import "./Structs.sol";
import "./PERFeeReceiver.sol";

import "forge-std/console.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";

contract PERMulticall {
    event ReceivedETH(address sender, uint256 amount);

    address _perOperator;
    mapping(address => uint256) _feeConfig;
    mapping(bytes32 => bool) _permissions;
    uint256 _defaultFee;

    /**
     * @notice PERMulticall constructor - Initializes a new multicall contract with given parameters
     *
     * @param perOperatorAddress: address of PER operator EOA
     * @param defaultFee: default fee split to be paid to the protocol whose permissioning is being used
     */
    constructor(address perOperatorAddress, uint256 defaultFee) {
        _perOperator = perOperatorAddress;
        _defaultFee = defaultFee;
    }

    /**
     * @notice getPEROperator function - returns the address of the PER operator
     */
    function getPEROperator() public view returns (address) {
        return _perOperator;
    }

    function isPermissioned(
        address profitReceiver,
        bytes calldata message
    ) public view returns (bool permissioned) {
        return _permissions[keccak256(abi.encode(profitReceiver, message))];
    }

    /**
     * @notice setFee function - sets the fee for a given fee recipient
     *
     * @param feeRecipient: address of the fee recipient for the contract being registered
     * @param feeSplit: amount of fee to be split with the protocol. 10**18 is 100%
     */
    function setFee(address feeRecipient, uint256 feeSplit) public {
        require(
            msg.sender == _perOperator,
            "only PER operator can set the fees"
        );
        _feeConfig[feeRecipient] = feeSplit;
    }

    function _isContract(address _addr) private view returns (bool) {
        uint32 size;
        assembly {
            size := extcodesize(_addr)
        }
        return (size > 0);
    }

    function _bytesToAddress(
        bytes memory bys
    ) private pure returns (address addr) {
        (addr, ) = abi.decode(bys, (address, bytes));
    }

    /**
     * @notice multicall function - performs a number of calls to external contracts in order
     *
     * @param permission: permission to allow for this call
     * @param contracts: ordered list of contracts to call into
     * @param data: ordered list of calldata to call with
     * @param bids: ordered list of bids; call i will fail if it does not pay PER operator at least bid i
     */
    function multicall(
        bytes calldata permission,
        address[] calldata contracts,
        bytes[] calldata data,
        uint256[] calldata bids
    ) public payable returns (MulticallStatus[] memory multicallStatuses) {
        require(
            msg.sender == _perOperator,
            "only PER operator can call this function"
        );
        require(
            permission.length >= 20,
            "permission size should be at least 20 bytes"
        );
        _permissions[keccak256(permission)] = true;
        multicallStatuses = new MulticallStatus[](data.length);

        uint256 totalBid = 0;
        for (uint256 i = 0; i < data.length; i++) {
            // try/catch will revert if call to searcher fails or if bid conditions not met
            try this.callWithBid(contracts[i], data[i], bids[i]) returns (
                bool success,
                bytes memory result
            ) {
                multicallStatuses[i].externalSuccess = success;
                multicallStatuses[i].externalResult = result;
            } catch Error(string memory reason) {
                multicallStatuses[i].multicallRevertReason = reason;
            }
            totalBid += bids[i];
        }

        // use the first 20 bytes of permission as fee receiver
        address feeReceiver = _bytesToAddress(permission);
        // transfer fee to the protocol
        uint256 protocolFee = _feeConfig[feeReceiver];
        if (protocolFee == 0) {
            protocolFee = _defaultFee;
        }
        uint256 feeProtocolNumerator = totalBid * protocolFee;
        if (feeProtocolNumerator > 0) {
            uint256 feeProtocol = feeProtocolNumerator /
                1000_000_000_000_000_000;
            if (_isContract(feeReceiver)) {
                PERFeeReceiver(feeReceiver).receiveAuctionProceedings{
                    value: feeProtocol
                }(permission);
            } else {
                payable(feeReceiver).transfer(feeProtocol);
            }
        }
        _permissions[keccak256(permission)] = false;
    }

    /**
     * @notice callWithBid function - contained call to function with check for bid invariant
     *
     * @param contractAddress: contract address to call into
     * @param data: calldata to call with
     * @param bid: bid to be paid; call will fail if it does not pay PER operator at least bid,
     */
    function callWithBid(
        address contractAddress,
        bytes calldata data,
        uint256 bid
    ) public payable returns (bool, bytes memory) {
        uint256 balanceInitEth = address(this).balance;

        (bool success, bytes memory result) = contractAddress.call(data);

        if (success) {
            uint256 balanceFinalEth = address(this).balance;

            // ensure that PER operator was paid at least bid ETH
            require(
                !(balanceFinalEth - balanceInitEth < bid) &&
                    !(balanceFinalEth < balanceInitEth),
                "invalid bid"
            );
        }

        return (success, result);
    }

    receive() external payable {
        emit ReceivedETH(msg.sender, msg.value);
    }
}

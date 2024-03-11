// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Errors.sol";
import "./Structs.sol";
import "./ExpressRelayFeeReceiver.sol";

import "forge-std/console.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";

contract ExpressRelay {
    event ReceivedETH(address sender, uint256 amount);

    // TODO: separate the notion operator into relayer and admin.
    // TODO: Relayer can submit transactions, admin can change relayers and set fees
    address _operator;
    mapping(address => uint256) _feeConfig;
    mapping(bytes32 => bool) _permissions;
    uint256 _defaultFee;

    /**
     * @notice ExpressRelay constructor - Initializes a new multicall contract with given parameters
     *
     * @param operator: address of express relay operator EOA
     * @param defaultFee: default fee split to be paid to the protocol whose permissioning is being used
     */
    constructor(address operator, uint256 defaultFee) {
        _operator = operator;
        _defaultFee = defaultFee;
    }

    /**
     * @notice getOperator function - returns the address of the express relay operator
     */
    function getOperator() public view returns (address) {
        return _operator;
    }

    function isPermissioned(
        address protocolFeeReceiver,
        bytes calldata permissionId
    ) public view returns (bool permissioned) {
        return
            _permissions[
                keccak256(abi.encode(protocolFeeReceiver, permissionId))
            ];
    }

    /**
     * @notice setFee function - sets the fee for a given fee recipient
     *
     * @param feeRecipient: address of the fee recipient for the contract being registered
     * @param feeSplit: amount of fee to be split with the protocol. 10**18 is 100%
     */
    function setFee(address feeRecipient, uint256 feeSplit) public {
        if (msg.sender != _operator) {
            revert Unauthorized();
        }
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
        // this does not assume the struct fields of the permission key
        addr = address(uint160(uint256(bytes32(bys))));
    }

    /**
     * @notice multicall function - performs a number of calls to external contracts in order
     *
     * @param permissionKey: permission to allow for this call
     * @param targetContracts: ordered list of contracts to call into
     * @param data: ordered list of calldata to call with
     * @param bidAmounts: ordered list of bids; call i will fail if it does not send this contract at least bid i
     */
    function multicall(
        bytes calldata permissionKey,
        address[] calldata targetContracts,
        bytes[] calldata data,
        uint256[] calldata bidAmounts
    ) public payable returns (MulticallStatus[] memory multicallStatuses) {
        if (msg.sender != _operator) {
            revert Unauthorized();
        }
        if (permissionKey.length < 20) {
            revert InvalidPermission();
        }

        _permissions[keccak256(permissionKey)] = true;
        multicallStatuses = new MulticallStatus[](data.length);

        uint256 totalBid = 0;
        for (uint256 i = 0; i < data.length; i++) {
            // try/catch will revert if call to searcher fails or if bid conditions not met
            try
                this.callWithBid(targetContracts[i], data[i], bidAmounts[i])
            returns (bool success, bytes memory result) {
                multicallStatuses[i].externalSuccess = success;
                multicallStatuses[i].externalResult = result;
            } catch Error(string memory reason) {
                multicallStatuses[i].multicallRevertReason = reason;
            }

            // only count bid if call was successful (and bid was paid out)
            if (multicallStatuses[i].externalSuccess) {
                totalBid += bidAmounts[i];
            }
        }

        // use the first 20 bytes of permission as fee receiver
        address feeReceiver = _bytesToAddress(permissionKey);
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
                ExpressRelayFeeReceiver(feeReceiver).receiveAuctionProceedings{
                    value: feeProtocol
                }(permissionKey);
            } else {
                payable(feeReceiver).transfer(feeProtocol);
            }
        }
        _permissions[keccak256(permissionKey)] = false;
    }

    /**
     * @notice callWithBid function - contained call to function with check for bid invariant
     *
     * @param targetContract: contract address to call into
     * @param data: calldata to call with
     * @param bid: bid to be paid; call will fail if it does not send this contract at least bid,
     */
    function callWithBid(
        address targetContract,
        bytes calldata data,
        uint256 bid
    ) public payable returns (bool, bytes memory) {
        uint256 balanceInitEth = address(this).balance;

        (bool success, bytes memory result) = targetContract.call(data);

        if (success) {
            uint256 balanceFinalEth = address(this).balance;

            // ensure that this contract was paid at least bid ETH
            require(
                (balanceFinalEth - balanceInitEth >= bid) &&
                    (balanceFinalEth >= balanceInitEth),
                "invalid bid"
            );
        }

        return (success, result);
    }

    receive() external payable {
        emit ReceivedETH(msg.sender, msg.value);
    }
}

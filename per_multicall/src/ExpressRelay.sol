// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Errors.sol";
import "./Structs.sol";

import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "@pythnetwork/express-relay-sdk-solidity/IExpressRelay.sol";
import "@pythnetwork/express-relay-sdk-solidity/IExpressRelayFeeReceiver.sol";

contract ExpressRelay is IExpressRelay {
    event ReceivedETH(address sender, uint256 amount);

    // TODO: Relayer can submit transactions, admin can change relayers and set fees
    // TODO: move this stuff over to ExpressRelayState.sol
    address _relayer;
    address _admin;
    mapping(address => uint256) _feeConfig;
    mapping(bytes32 => bool) _permissions;
    uint256 _feeSplitProtocolDefault;
    uint256 _feeSplitRelayer;
    uint256 _feeSplitPrecision = 10 ** 18;

    /**
     * @notice ExpressRelay constructor - Initializes a new multicall contract with given parameters
     *
     * @param admin: address of admin of express relay
     * @param relayer: address of relayer EOA
     * @param feeSplitProtocolDefault: default fee split to be paid to the protocol whose permissioning is being used
     * @param feeSplitRelayer: split of the non-protocol fees to be paid to the relayer
     */
    constructor(
        address admin,
        address relayer,
        uint256 feeSplitProtocolDefault,
        uint256 feeSplitRelayer
    ) {
        _admin = admin;
        _relayer = relayer;

        if (feeSplitProtocolDefault > _feeSplitPrecision) {
            revert InvalidFeeSplit();
        }
        _feeSplitProtocolDefault = feeSplitProtocolDefault;

        if (feeSplitRelayer > _feeSplitPrecision) {
            revert InvalidFeeSplit();
        }
        _feeSplitRelayer = feeSplitRelayer;
    }

    modifier onlyAdmin() {
        if (msg.sender != _admin) {
            revert Unauthorized();
        }
        _;
    }

    modifier onlyRelayer() {
        if (msg.sender != _relayer) {
            revert Unauthorized();
        }
        _;
    }

    /**
     * @notice setRelayer function - sets the relayer
     *
     * @param relayer: address of the relayer to be set
     */
    function setRelayer(address relayer) public onlyAdmin {
        _relayer = relayer;
    }

    /**
     * @notice getRelayer function - returns the address of the relayer
     */
    function getRelayer() public view returns (address) {
        return _relayer;
    }

    /**
     * @notice setFeeProtocolDefault function - sets the default fee split for the protocol
     *
     * @param feeSplit: split of fee to be sent to the protocol. 10**18 is 100%
     */
    function setFeeProtocolDefault(uint256 feeSplit) public onlyAdmin {
        if (feeSplit > _feeSplitPrecision) {
            revert InvalidFeeSplit();
        }
        _feeSplitProtocolDefault = feeSplit;
    }

    /**
     * @notice getFeeProtocolDefault function - returns the default fee split for the protocol
     */
    function getFeeProtocolDefault() public view returns (uint256) {
        return _feeSplitProtocolDefault;
    }

    /**
     * @notice setFeeProtocol function - sets the fee split for a given protocol fee recipient
     *
     * @param feeRecipient: address of the fee recipient for the protocol
     * @param feeSplit: split of fee to be sent to the protocol. 10**18 is 100%
     */
    function setFeeProtocol(
        address feeRecipient,
        uint256 feeSplit
    ) public onlyAdmin {
        if (feeSplit > _feeSplitPrecision) {
            revert InvalidFeeSplit();
        }
        _feeConfig[feeRecipient] = feeSplit;
    }

    /**
     * @notice getFeeProtocol function - returns the fee split for a given protocol fee recipient
     *
     * @param feeRecipient: address of the fee recipient for the protocol
     */
    function getFeeProtocol(
        address feeRecipient
    ) public view returns (uint256) {
        uint256 feeProtocol = _feeConfig[feeRecipient];
        if (feeProtocol == 0) {
            feeProtocol = _feeSplitProtocolDefault;
        }
        return feeProtocol;
    }

    /**
     * @notice setFeeRelayer function - sets the fee split for the relayer
     *
     * @param feeSplit: split of remaining fee (after subtracting protocol fee) to be sent to the relayer. 10**18 is 100%
     */
    function setFeeRelayer(uint256 feeSplit) public onlyAdmin {
        if (feeSplit > _feeSplitPrecision) {
            revert InvalidFeeSplit();
        }
        _feeSplitRelayer = feeSplit;
    }

    /**
     * @notice getFeeRelayer function - returns the fee split for the relayer
     */
    function getFeeRelayer() public view returns (uint256) {
        return _feeSplitRelayer;
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
     * @param targetCalldata: ordered list of calldata to call the targets with
     * @param bidAmounts: ordered list of bids; call i will fail if it does not send this contract at least bid i
     */
    function multicall(
        bytes calldata permissionKey,
        address[] calldata targetContracts,
        bytes[] calldata targetCalldata,
        uint256[] calldata bidAmounts
    )
        public
        payable
        onlyRelayer
        returns (MulticallStatus[] memory multicallStatuses)
    {
        if (permissionKey.length < 20) {
            revert InvalidPermission();
        }

        _permissions[keccak256(permissionKey)] = true;
        multicallStatuses = new MulticallStatus[](targetCalldata.length);

        uint256 totalBid = 0;
        for (uint256 i = 0; i < targetCalldata.length; i++) {
            // try/catch will revert if call to searcher fails or if bid conditions not met
            try
                this.callWithBid(
                    targetContracts[i],
                    targetCalldata[i],
                    bidAmounts[i]
                )
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
        uint256 feeSplitProtocol = _feeConfig[feeReceiver];
        if (feeSplitProtocol == 0) {
            feeSplitProtocol = _feeSplitProtocolDefault;
        }
        uint256 feeProtocol;
        uint256 feeProtocolNumerator = totalBid * feeSplitProtocol;
        if (feeProtocolNumerator > 0) {
            feeProtocol = feeProtocolNumerator / _feeSplitPrecision;
            if (_isContract(feeReceiver)) {
                IExpressRelayFeeReceiver(feeReceiver).receiveAuctionProceedings{
                    value: feeProtocol
                }(permissionKey);
            } else {
                payable(feeReceiver).transfer(feeProtocol);
            }
        }
        _permissions[keccak256(permissionKey)] = false;

        // pay the relayer
        uint256 feeRelayerNumerator = (totalBid - feeProtocol) *
            _feeSplitRelayer;
        if (feeRelayerNumerator > 0) {
            uint256 feeRelayer = feeRelayerNumerator / _feeSplitPrecision;
            payable(_relayer).transfer(feeRelayer);
        }
    }

    /**
     * @notice callWithBid function - contained call to function with check for bid invariant
     *
     * @param targetContract: contract address to call into
     * @param targetCalldata: calldata to call the target with
     * @param bid: bid to be paid; call will fail if it does not send this contract at least bid,
     */
    function callWithBid(
        address targetContract,
        bytes calldata targetCalldata,
        uint256 bid
    ) public payable returns (bool, bytes memory) {
        uint256 balanceInitEth = address(this).balance;

        (bool success, bytes memory result) = targetContract.call(
            targetCalldata
        );

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

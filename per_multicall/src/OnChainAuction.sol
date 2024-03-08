// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Errors.sol";
import "forge-std/console.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";
import "./Structs.sol";

error BidTooLow();
error BidAlreadyExists();

struct Bid {
    address bidder;
    uint256 amount;
    uint256 blockNumber;
}

contract AuctionManager {
    mapping(bytes32 => Bid) _bids;
    mapping(address => uint256) _feeConfig;
    address _admin;

    function bid(bytes32 permissionKeyHash) public payable {
        Bid memory currentBid = _bids[permissionKeyHash];
        if (currentBid.bidder != address(0)) {
            if (currentBid.amount > msg.value) {
                revert BidTooLow();
            } else {
                payable(currentBid.bidder).transfer(currentBid.amount); // return the previous bid
            }
        }
        _bids[permissionKeyHash] = Bid(msg.sender, msg.value, block.number);
    }

    mapping(bytes32 => bool) _permissions;

    /**
     * @notice constructor - Initializes a new auction manager with an admin used for setting the fees
     *
     * @param admin: admin of the auction manager
     */
    constructor(address admin) {
        _admin = admin;
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
        require(msg.sender == _admin, "only the admin can set the fees");
        _feeConfig[feeRecipient] = feeSplit;
    }

    function execute(
        bytes[] calldata permissions,
        address contractAddress,
        bytes calldata data
    ) public payable {
        for (uint256 i = 0; i < permissions.length; i++) {
            bytes32 permissionKeyHash = keccak256(permissions[i]);
            require(
                _bids[permissionKeyHash].bidder == msg.sender,
                "not the highest bidder"
            );
            _permissions[permissionKeyHash] = true;
        }
        (bool success, ) = contractAddress.call(data);
        require(success, "contract call failed");
        for (uint256 i = 0; i < permissions.length; i++) {
            _permissions[keccak256(permissions[i])] = false;
        }
    }

    function settleBids(bytes calldata permissionKey) public {
        address feeReceiver = abi.decode(permissionKey[0:32], (address));
        bytes32 permissionKeyHash = keccak256(permissionKey);
        Bid memory currentBid = _bids[permissionKeyHash];
        require(currentBid.blockNumber < block.number - 1000, "not expired"); // we assume an auction is over after 1000 blocks

        uint256 feeProtocolNumerator = currentBid.amount *
            _feeConfig[feeReceiver];
        if (feeProtocolNumerator > 0) {
            uint256 feeProtocol = feeProtocolNumerator /
                1000_000_000_000_000_000;
            payable(feeReceiver).transfer(feeProtocol);
        }
        delete _bids[permissionKeyHash];
    }
}

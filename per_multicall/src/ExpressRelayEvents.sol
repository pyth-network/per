// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Structs.sol";

contract ExpressRelayEvents {
    event ReceivedETH(address indexed sender, uint256 amount);
    event MulticallIssued(
        bytes indexed permissionKey,
        uint256 indexed multicallIndex,
        bytes16 indexed bidId,
        uint256 bidAmount,
        MulticallStatus multicallStatus
    );
    event RelayerSet(address relayer);
    event RelayerSubwalletAdded(address indexed relayer, address subwallet);
    event RelayerSubwalletRemoved(address indexed relayer, address subwallet);
    event FeeProtocolDefaultSet(uint256 feeSplit);
    event FeeProtocolSet(address indexed feeRecipient, uint256 feeSplit);
    event FeeRelayerSet(uint256 feeSplit);
}

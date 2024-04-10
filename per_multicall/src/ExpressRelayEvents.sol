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
}

// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

interface PERFeeReceiver {
    function receiveAuctionProceedings(
        bytes calldata permissionKey
    ) external payable;
}

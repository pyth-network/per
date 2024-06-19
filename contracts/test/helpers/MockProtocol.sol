// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import "@pythnetwork/express-relay-sdk-solidity/IExpressRelayFeeReceiver.sol";
import "@pythnetwork/express-relay-sdk-solidity/IExpressRelay.sol";

// Signature: 0x2003013a
error MockProtocolUnauthorized();

// Signature: 0xb805294a
error MockProtocolFail();

contract MockProtocol is IExpressRelayFeeReceiver {
    address _expressRelay;
    address _feeReceiver;
    event MockProtocolReceivedAuctionProceedings(bytes permissionKey);

    constructor(address expressRelay) {
        _expressRelay = expressRelay;
        _feeReceiver = address(this);
    }

    function setFeeReceiver(address feeReceiver) public {
        _feeReceiver = feeReceiver;
    }

    function execute() public payable {
        if (
            !IExpressRelay(_expressRelay).isPermissioned(
                _feeReceiver,
                abi.encode(uint256(0))
            )
        ) {
            revert MockProtocolUnauthorized();
        }
    }

    function executeFail() public payable {
        revert MockProtocolFail();
    }

    function executePermissionless() public payable {}

    function receiveAuctionProceedings(
        bytes calldata permissionKey
    ) external payable {
        emit MockProtocolReceivedAuctionProceedings(permissionKey);
    }
}

contract MockTarget {
    address _expressRelay;
    address _mockProtocol;

    constructor(address expressRelay, address mockProtocol) {
        _expressRelay = expressRelay;
        _mockProtocol = mockProtocol;
    }

    function passThrough(uint256 bid) public payable {
        MockProtocol(_mockProtocol).execute();
        payable(_expressRelay).transfer(bid);
    }

    function passThroughFail(uint256 bid) public payable {
        MockProtocol(_mockProtocol).executeFail();
        payable(_expressRelay).transfer(bid);
    }

    function passThroughPermissionless(uint256 bid) public payable {
        MockProtocol(_mockProtocol).executePermissionless();
        payable(_expressRelay).transfer(bid);
    }
}

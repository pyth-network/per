// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import "./Structs.sol";

contract ExpressRelayGovernanceEvents {
    event RelayerSet(address relayer);
    event RelayerSubwalletAdded(address indexed relayer, address subwallet);
    event RelayerSubwalletRemoved(address indexed relayer, address subwallet);
    event FeeProtocolDefaultSet(uint256 feeSplit);
    event FeeProtocolSet(address indexed feeRecipient, uint256 feeSplit);
    event FeeRelayerSet(uint256 feeSplit);
}

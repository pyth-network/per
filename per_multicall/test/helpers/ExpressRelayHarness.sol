// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import {ExpressRelayUpgradable} from "src/express-relay/ExpressRelayUpgradable.sol";

contract ExpressRelayHarness is ExpressRelayUpgradable {
    function exposed_setFeeSplitPrecision() external {
        return setFeeSplitPrecision();
    }

    function exposed_validateFeeSplit(uint256 feeSplit) external view {
        return validateFeeSplit(feeSplit);
    }

    function exposed_isContract(address addr) external view returns (bool) {
        return isContract(addr);
    }

    function exposed_bytesToAddress(
        bytes memory bys
    ) external pure returns (address addr) {
        return bytesToAddress(bys);
    }
}

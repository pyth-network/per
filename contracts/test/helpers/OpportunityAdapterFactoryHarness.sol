// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import {OpportunityAdapterFactory} from "src/opportunity-adapter/OpportunityAdapterFactory.sol";

contract OpportunityAdapterFactoryHarness is OpportunityAdapterFactory {
    constructor(
        address expressRelay,
        address weth,
        address permit2
    ) OpportunityAdapterFactory(expressRelay, weth, permit2) {}

    function exposed_isContract(address addr) external returns (bool) {
        return _isContract(addr);
    }
}

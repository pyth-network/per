// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

interface IOpportunityAdapterFactory {
    function parameters()
        external
        view
        returns (
            address expressRelay,
            address weth,
            address permit2,
            address owner
        );
}

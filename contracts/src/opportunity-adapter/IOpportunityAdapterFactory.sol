// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

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

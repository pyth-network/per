// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "@pythnetwork/pyth-sdk-solidity/MockPyth.sol";

contract PriceHelpers {
    function getDebtLiquidationPrice(
        uint256 amountCollateral,
        uint256 amountDebt,
        uint256 thresholdHealthRatio,
        uint256 healthPrecision,
        int64 priceCollateral
    ) public pure returns (int64) {
        return
            int64(
                uint64(
                    (amountCollateral *
                        uint256(uint64(priceCollateral)) *
                        100 *
                        healthPrecision) /
                        (amountDebt * thresholdHealthRatio) +
                        1
                )
            );
    }

    function getCollateralLiquidationPrice(
        uint256 amountCollateral,
        uint256 amountDebt,
        uint256 thresholdHealthRatio,
        uint256 healthPrecision,
        int64 priceDebt
    ) public pure returns (int64) {
        return
            int64(
                uint64(
                    (amountDebt *
                        uint256(uint64(priceDebt)) *
                        thresholdHealthRatio) /
                        (amountCollateral * 100 * healthPrecision) -
                        1
                )
            );
    }

    function createPriceFeedUpdateSimple(
        MockPyth mockPyth,
        bytes32 id,
        int64 price,
        int32 expo
    ) public view returns (bytes memory) {
        return
            mockPyth.createPriceFeedUpdateData(
                id,
                price,
                1, // bogus confidence
                expo,
                1, // bogus ema price
                1, // bogus ema confidence
                uint64(block.timestamp),
                0 // bogus previous timestamp
            );
    }
}

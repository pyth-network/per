// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

struct OracleState { 
   uint256 price;
   uint256 timestamp;
}

struct Vault {
   address tokenCollateral;
   address tokenDebt;
   uint256 amountCollateral;
   uint256 amountDebt;
   uint256 minHealthRatio;
   uint256 precisionRatio;
}

struct FeeMetadata {
   uint256 feeSplitProtocol;
   uint256 feeSplitPrecision;
}
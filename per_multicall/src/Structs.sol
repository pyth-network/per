// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
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
    uint256 minHealthRatio; // 10**18 is 100%
    uint256 minPermissionLessHealthRatio;
    bytes32 tokenIDCollateral;
    bytes32 tokenIDDebt;
}

struct TokenAmount {
    address token;
    uint256 amount;
}

struct MulticallStatus {
    bool externalSuccess;
    bytes externalResult;
    string multicallRevertReason;
}

struct LiquidationCallParams {
    TokenAmount[] sellTokens;
    TokenAmount[] buyTokens;
    address liquidator;
    address contractAddress;
    bytes data;
    uint256 value;
    uint256 validUntil;
    uint256 bidAmount;
    bytes signature;
}

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
    uint256 minPermissionlessHealthRatio;
    bytes32 tokenIdCollateral;
    bytes32 tokenIdDebt;
}

struct TokenAmount {
    address token;
    uint256 amount;
}

struct MulticallData {
    bytes16 bidId;
    address targetContract;
    bytes targetCalldata;
    uint256 bidAmount;
}

struct MulticallStatus {
    bool externalSuccess;
    bytes externalResult;
    string multicallRevertReason;
}

struct ExecutionParams {
    TokenAmount[] sellTokens;
    TokenAmount[] buyTokens;
    address executor;
    address targetContract;
    bytes targetCalldata;
    uint256 targetCallValue;
    uint256 validUntil;
    uint256 bidAmount;
    bytes signature;
}

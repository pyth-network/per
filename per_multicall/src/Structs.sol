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

struct ExecutionWitness {
    TokenAmount[] buyTokens;
    address executor;
    address targetContract;
    bytes targetCalldata;
    uint256 targetCallValue;
    uint256 bidAmount;
}

struct ExecutionParams {
    PermitBatchTransferFrom permit;
    ExecutionWitness witness;
}

/// @notice The token and amount details for a transfer signed in the permit transfer signature
struct TokenPermissions {
    // ERC20 token address
    address token;
    // the maximum amount that can be spent
    uint256 amount;
}

/// @notice Used to reconstruct the signed permit message for multiple token transfers
/// @dev Do not need to pass in spender address as it is required that it is msg.sender
/// @dev Note that a user still signs over a spender address
struct PermitBatchTransferFrom {
    // the tokens and corresponding amounts permitted for a transfer
    TokenPermissions[] permitted;
    // a unique value for every token owner's signature to prevent signature replays
    uint256 nonce;
    // deadline on the permit signature
    uint256 deadline;
}

/// @notice Specifies the recipient address and amount for batched transfers.
/// @dev Recipients and amounts correspond to the index of the signed token permissions array.
/// @dev Reverts if the requested amount is greater than the permitted signed amount.
struct SignatureTransferDetails {
    // recipient address
    address to;
    // spender requested amount
    uint256 requestedAmount;
}

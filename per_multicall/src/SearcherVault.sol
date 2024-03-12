// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "openzeppelin-contracts/contracts/utils/Strings.sol";
import "forge-std/console.sol";

import "./Errors.sol";
import "./Structs.sol";
import "./TokenVault.sol";
import "./ExpressRelay.sol";
import "./SigVerify.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "@pythnetwork/pyth-sdk-solidity/MockPyth.sol";

contract SearcherVault is SigVerify {
    event ReceivedETH(address, uint);

    address public immutable expressRelay;
    address public immutable owner;
    address public immutable tokenVault;

    mapping(bytes => bool) _signatureUsed;

    /**
     * @notice Searcher constructor - Initializes a new searcher contract with given parameters around token vault protocol
     *
     * @param expressRelayAddress: address of express relay
     * @param protocolAddress: address of token vault protocol contract
     */
    constructor(address expressRelayAddress, address protocolAddress) {
        owner = msg.sender;
        expressRelay = expressRelayAddress;
        tokenVault = protocolAddress;
    }

    /**
     * @notice doLiquidate function - liquidates a vault through express relay
     *
     * @param vaultID: ID of the vault to be liquidated
     * @param bid: size of the bid to pay to express relay
     * @param validUntil: timestamp at which signatureSearcher is no longer valid
     * @param updateData: data to update price feed with
     * @param signatureSearcher: signature of the vaultID and bid, signed by the searcher's EOA, to be verified if msg.sender is express relay
     */
    function doLiquidate(
        uint256 vaultID,
        uint256 bid,
        uint256 validUntil,
        bytes calldata updateData,
        bytes calldata signatureSearcher
    ) public payable {
        if (msg.sender != expressRelay && msg.sender != owner) {
            revert Unauthorized();
        }

        if (msg.sender == expressRelay) {
            bool validSignatureSearcher = verifyCalldata(
                owner,
                abi.encode(vaultID, bid, validUntil),
                signatureSearcher
            );
            if (!validSignatureSearcher) {
                revert InvalidSearcherSignature();
            }
            if (block.timestamp > validUntil) {
                revert ExpiredSignature();
            }
            if (_signatureUsed[signatureSearcher]) {
                revert SignatureAlreadyUsed();
            }
        }

        address payable vaultContract = payable(tokenVault);

        Vault memory vault = TokenVault(vaultContract).getVault(vaultID);

        address tokenDebt = vault.tokenDebt;
        uint256 tokenAmount = vault.amountDebt;

        IERC20(tokenDebt).approve(vaultContract, tokenAmount);
        bytes[] memory updateDatas = new bytes[](1);
        updateDatas[0] = updateData;
        TokenVault(vaultContract).liquidateWithPriceUpdate(
            vaultID,
            updateDatas
        );
        if (bid > 0) {
            payable(expressRelay).transfer(bid);
        }

        // mark signature as used
        _signatureUsed[signatureSearcher] = true;
    }

    function withdrawEth(uint256 amount) public {
        if (msg.sender != owner) {
            revert Unauthorized();
        }
        payable(owner).transfer(amount);
    }

    receive() external payable {
        emit ReceivedETH(msg.sender, msg.value);
    }
}

// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "openzeppelin-contracts/contracts/utils/Strings.sol";

import "./Errors.sol";
import "./Structs.sol";
import "./TokenVault.sol";
import "./PERMulticall.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";

contract Searcher {
    event ReceivedETH(address, uint);

    address public immutable perMulticall;
    address public immutable owner;
    address public immutable protocol;

    /**
     * @notice Searcher constructor - Initializes a new searcher contract with given parameters around token vault protocol
     * 
     * @param perMulticallAddress: address of PER contract
     * @param protocolAddress: address of token vault protocol contract
     */
    constructor(address perMulticallAddress, address protocolAddress) {
        owner = msg.sender;
        perMulticall = perMulticallAddress;
        protocol = protocolAddress;
    }

    /**
     * @notice doLiquidate function - liquidates a vault (slow path)
     * 
     * @param vaultID: ID of the vault to be liquidated
     */
    function doLiquidate(
        uint256 vaultID
    ) public payable {
        revert NotImplemented();

        /// This method is called to liquidate a vault on the TokenVault protocol via the slow path
        /// function. It does not require any PER interaction and can be called by the searcher 
        /// directly. It determines the outstanding debt of the vault and approves the TokenVault
        /// to spend that amount of debt tokens custodied by the searcher. It then calls into the
        /// TokenVault contract's liquidate method. 
    }

    /**
     * @notice doLiquidateFast function - liquidates a vault (fast path)
     * 
     * @param signaturePER: signature of the block number and contract address, signed by the PER operator
     * @param vaultID: ID of the vault to be liquidated
     * @param signatureSearcher: signature of the vaultID calldata and block number, signed by the searcher's EOA
     * @param bid: size of the bid to pay to PER operator
     */
    function doLiquidateFast(
        bytes memory signaturePER,
        uint256 vaultID, 
        bytes memory signatureSearcher,
        uint256 bid
    ) public payable {
        revert NotImplemented();

        /// This method is called to liquidate a vault on the TokenVault protocol via the fast path
        /// function. It requires PER interaction and can only be called by the PER operator. It
        /// requires a signature from the searcher's EOA to verify that the searcher has signed off
        /// on this liquidation along with the bid amount. It also requires that tx.origin is the PER
        /// operator, in order to prevent anyone else from copying the signatures and calling this.
        /// After checking these conditions, the logic proceeds the same as the doLiquidate method
        /// above, except that it calls into the TokenVault contract's liquidateFast (as opposed to
        /// liquidate) method. At the end, it transfers the bid amount to the PER operator.
    }

    receive() external payable {
        emit ReceivedETH(msg.sender, msg.value);
    }
}

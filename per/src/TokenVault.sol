// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./Errors.sol";
import "forge-std/StdMath.sol";
import "./PERMulticall.sol";
import "./MockOracle.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";

contract TokenVault is MockOracle {
    using SafeERC20 for IERC20;

    event VaultReceivedETH(address sender, uint256 amount);

    uint256 _slowDelay;
    uint256 _nVaults;
    address public immutable perMulticall;
    mapping(uint256 => Vault) _vaults;

    /**
     * @notice TokenVault constructor - Initializes a new token vault contract with given parameters
     * 
     * @param delay: number of seconds to delay slow path by (i.e. min staleness of slow path oracle feed)
     * @param perMulticallAddress: address of PER contract
     */
    constructor(uint256 delay, address perMulticallAddress) {
        _slowDelay = delay;
        _nVaults = 0;
        perMulticall = perMulticallAddress;
    }

    /**
     * @notice _checkOvercollateralized function - checks if a vault is overcollateralized at a given delay
     * 
     * @param vault: vault struct containing vault parameters
     * @param delay: staleness at which to check overcollateralization
     */
    function _checkOvercollateralized(
        Vault memory vault,
        uint256 delay
    ) internal view returns (bool) {
        revert NotImplemented();

        /// This method is called to check if the vault in question is overcollateralized.
        /// This takes in a vault struct and a delay value that indicates which oracle
        /// price feed to check.
    }

    /**
     * @notice createVault function - creates a new vault
     * 
     * @param tokenCollateral: address of the collateral token of the vault
     * @param tokenDebt: address of the debt token of the vault
     * @param amountCollateral: amount of collateral tokens in the vault
     * @param amountDebt: amount of debt tokens in the vault
     * @param minHealthRatio: minimum health ratio of the vault, in precision units
     * @param precisionRatio: precision ratio of the vault
     */
    function createVault(
        address tokenCollateral,
        address tokenDebt,
        uint256 amountCollateral,
        uint256 amountDebt,
        uint256 minHealthRatio,
        uint256 precisionRatio
    ) public returns (uint256) {
        revert NotImplemented();

        /// This method is called to create a new vault with collateral of denomination 
        /// tokenCollateral and outstanding debt of denomination tokenDebt. This method creates a new
        /// vault struct and checks if it is overcollateralized via both the slow and fast oracle
        /// feeds, for safety reasons. If not, it will revert; otherwise, it will transfer the
        /// collateral token from the sender to the vault contract, and transfer the debt token from
        /// the vault contract to the sender. It will then add the vault struct to the vaults mapping.
    }

    /**
     * @notice updateVault function - updates a vault's collateral and debt amounts
     * 
     * @param vaultID: ID of the vault to be updated
     * @param deltaCollateral: delta change to collateral amount (+ means adding collateral tokens, - means removing collateral tokens)
     * @param deltaDebt: delta change to debt amount (+ means withdrawing debt tokens from protocol, - means resending debt tokens to protocol)
     */
    function updateVault(
        uint256 vaultID,
        int256 deltaCollateral,
        int256 deltaDebt
    ) public {
        revert NotImplemented();
        
        /// This method allows one to update a vault's collateral and debt amounts. It examines the
        /// intended changes to the vault's collateral and debt, checking if the token transfers
        /// are valid and if the vault will remain overcollateralized at both the slow and fast 
        /// oracle feeds. If not, it will revert; otherwise, it will update the positions by
        /// transferring tokens and updating the vault struct.
    }

    /**
     * @notice getVault function - getter function to get a vault's parameters
     * 
     * @param vaultID: ID of the vault
     */
    function getVault(uint256 vaultID) public view returns (Vault memory) {
        return _vaults[vaultID];
    }

    /**
     * @notice liquidate function - liquidates a vault (slow path)
     * 
     * @param vaultID: ID of the vault to be liquidated
     */
    function liquidate(uint256 vaultID) public {
        revert NotImplemented();

        /// This method performs a simplified liquidation operation via the slow oracle feed's 
        /// prices. It checks if the vault is undercollateralized at the slow oracle feed. If not, it
        /// will revert; if it is, it will allow the liquidator to transfer the outstanding debt to
        /// the contract and receive the collateral in return. It will then zero out the vault's
        /// stats in the vault struct.
    }

    /**
     * @notice liquidateFast function - liquidates a vault (fast path, can only be called by tx origin PER operator)
     * 
     * @param vaultID: ID of the vault to be liquidated
     * @param signaturePER: PER operator signature object
     */
    function liquidateFast(uint256 vaultID, bytes memory signaturePER) public {
        revert NotImplemented();

        /// This method performs a simplified liquidation operation via the fast oracle feed's 
        /// prices. It performs the same logic as specified above for the slow oracle feed (of course
        /// checking collateralization at the fast oracle feed). In addition, it checks that:
        /// 1) tx.origin is the PER operator, i.e. the call originated with the PER operator
        /// 2) the PER operator's signature matches this contract address and the current block 
        /// number--it can make this check by calling into the PERSignatureValidation contract's
        /// validateSignaturePER method
        /// If either of these checks fail, it will revert.
    }

    receive() external payable { 
        emit VaultReceivedETH(msg.sender, msg.value);
    }
}

// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./Errors.sol";
import "forge-std/console.sol";
import "forge-std/StdMath.sol";
import "./Structs.sol";
import "./PERMulticall.sol";
import "./PERFeeReceiver.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";

import {MyToken} from "./MyToken.sol";

import "@pythnetwork/pyth-sdk-solidity/PythStructs.sol";
import "@pythnetwork/pyth-sdk-solidity/MockPyth.sol";

contract TokenVault is PERFeeReceiver {
    using SafeERC20 for IERC20;

    event VaultReceivedETH(address sender, uint256 amount, bytes permissionKey);

    uint256 _nVaults;
    address public immutable perMulticall;
    mapping(uint256 => Vault) _vaults;
    address _oracle;

    /**
     * @notice TokenVault constructor - Initializes a new token vault contract with given parameters
     *
     * @param perMulticallAddress: address of PER contract
     * @param oracleAddress: address of the oracle contract
     */
    constructor(address perMulticallAddress, address oracleAddress) {
        _nVaults = 0;
        perMulticall = perMulticallAddress;
        _oracle = oracleAddress;
    }

    /**
     * @notice getPrice function - retrieves price of a given token from the oracle
     *
     * @param id: price feed ID of the token
     */
    function _getPrice(
        bytes32 id
    ) internal view returns (PythStructs.Price memory) {
        MockPyth oracle = MockPyth(payable(_oracle));
        PythStructs.PriceFeed memory priceFeed = oracle.queryPriceFeed(id);
        return priceFeed.price;
    }

    function getOracle() public view returns (address) {
        return _oracle;
    }

    /**
     * @notice getVaultHealth function - calculates vault collateral/debt ratio
     *
     * @param vaultID: ID of the vault for which to calculate health
     */
    function getVaultHealth(uint256 vaultID) public view returns (uint256) {
        Vault memory vault = _vaults[vaultID];
        return _getVaultHealth(vault);
    }

    /**
     * @notice _getVaultHealth function - calculates vault collateral/debt ratio
     *
     * @param vault: vault struct containing vault parameters
     */
    function _getVaultHealth(
        Vault memory vault
    ) internal view returns (uint256) {
        int64 priceCollateral = _getPrice(vault.tokenIDCollateral).price;
        int64 priceDebt = _getPrice(vault.tokenIDDebt).price;

        require(priceCollateral >= 0, "collateral price is negative");
        require(priceDebt >= 0, "debt price is negative");

        uint256 valueCollateral = uint256(uint64(priceCollateral)) *
            vault.amountCollateral;
        uint256 valueDebt = uint256(uint64(priceDebt)) * vault.amountDebt;

        return (valueCollateral * 1_000_000_000_000_000_000) / valueDebt;
    }

    /**
     * @notice createVault function - creates a vault
     *
     * @param tokenCollateral: address of the collateral token of the vault
     * @param tokenDebt: address of the debt token of the vault
     * @param amountCollateral: amount of collateral tokens in the vault
     * @param amountDebt: amount of debt tokens in the vault
     * @param minHealthRatio: minimum health ratio of the vault, 10**18 is 100%
     * @param minPermissionLessHealthRatio: minimum health ratio of the vault before permissionless liquidations are allowed. This should be less than minHealthRatio
     * @param tokenIDCollateral: price feed ID of the collateral token
     * @param tokenIDDebt: price feed ID of the debt token
     */
    function createVault(
        address tokenCollateral,
        address tokenDebt,
        uint256 amountCollateral,
        uint256 amountDebt,
        uint256 minHealthRatio,
        uint256 minPermissionLessHealthRatio,
        bytes32 tokenIDCollateral,
        bytes32 tokenIDDebt
    ) public returns (uint256) {
        Vault memory vault = Vault(
            tokenCollateral,
            tokenDebt,
            amountCollateral,
            amountDebt,
            minHealthRatio,
            minPermissionLessHealthRatio,
            tokenIDCollateral,
            tokenIDDebt
        );
        require(
            minPermissionLessHealthRatio <= minHealthRatio,
            "minPermissionLessHealthRatio must be less than or equal to minHealthRatio"
        );
        if (_getVaultHealth(vault) < vault.minHealthRatio) {
            revert UncollateralizedVaultCreation();
        }

        IERC20(vault.tokenCollateral).safeTransferFrom(
            msg.sender,
            address(this),
            vault.amountCollateral
        );
        IERC20(vault.tokenDebt).safeTransfer(msg.sender, vault.amountDebt);

        _vaults[_nVaults] = vault;
        _nVaults += 1;

        return _nVaults;
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
        Vault memory vault = _vaults[vaultID];

        uint256 qCollateral = stdMath.abs(deltaCollateral);
        uint256 qDebt = stdMath.abs(deltaDebt);

        bool withdrawExcessiveCollateral = (deltaCollateral < 0) &&
            (qCollateral > vault.amountCollateral);

        if (withdrawExcessiveCollateral) {
            revert InvalidVaultUpdate();
        }

        uint256 futureCollateral = (deltaCollateral >= 0)
            ? (vault.amountCollateral + qCollateral)
            : (vault.amountCollateral - qCollateral);
        uint256 futureDebt = (deltaDebt >= 0)
            ? (vault.amountDebt + qDebt)
            : (vault.amountDebt - qDebt);

        vault.amountCollateral = futureCollateral;
        vault.amountDebt = futureDebt;

        if (_getVaultHealth(vault) < vault.minHealthRatio) {
            revert InvalidVaultUpdate();
        }

        // update collateral position
        if (deltaCollateral >= 0) {
            // sender adds more collateral to their vault
            IERC20(vault.tokenCollateral).safeTransferFrom(
                msg.sender,
                address(this),
                qCollateral
            );
            _vaults[vaultID].amountCollateral += qCollateral;
        } else {
            // sender takes back collateral from their vault
            IERC20(vault.tokenCollateral).safeTransfer(msg.sender, qCollateral);
            _vaults[vaultID].amountCollateral -= qCollateral;
        }

        // update debt position
        if (deltaDebt >= 0) {
            // sender takes out more debt position
            IERC20(vault.tokenDebt).safeTransfer(msg.sender, qDebt);
            _vaults[vaultID].amountDebt += qDebt;
        } else {
            // sender sends back debt tokens
            IERC20(vault.tokenDebt).safeTransferFrom(
                msg.sender,
                address(this),
                qDebt
            );
            _vaults[vaultID].amountDebt -= qDebt;
        }
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
     * @notice _updatePriceFeeds function - updates the specified price feeds with given data
     *
     * @param updateData: data to update price feeds with
     */
    function _updatePriceFeeds(bytes[] calldata updateData) internal {
        MockPyth oracle = MockPyth(payable(_oracle));
        oracle.updatePriceFeeds{value: msg.value}(updateData);
    }

    /**
     * @notice liquidate function - liquidates a vault
     *
     * @param vaultID: ID of the vault to be liquidated
     */
    function liquidate(uint256 vaultID) public {
        Vault memory vault = _vaults[vaultID];
        uint256 vaultHealth = _getVaultHealth(vault);
        if (vaultHealth >= vault.minHealthRatio) {
            revert InvalidLiquidation();
        }

        if (
            vaultHealth >= vault.minPermissionLessHealthRatio &&
            !PERMulticall(payable(perMulticall)).isPermissioned(
                address(this),
                abi.encode(vaultID)
            )
        ) {
            revert InvalidLiquidation();
        }

        IERC20(vault.tokenDebt).transferFrom(
            msg.sender,
            address(this),
            vault.amountDebt
        );
        IERC20(vault.tokenCollateral).transfer(
            msg.sender,
            vault.amountCollateral
        );

        _vaults[vaultID].amountCollateral = 0;
        _vaults[vaultID].amountDebt = 0;
    }

    /**
     * @notice liquidateWithPriceUpdate function - liquidates a vault after updating the specified price feeds with given data
     *
     * @param vaultID: ID of the vault to be liquidated
     * @param updateData: data to update price feeds with
     */
    function liquidateWithPriceUpdate(
        uint256 vaultID,
        bytes[] calldata updateData
    ) external payable {
        _updatePriceFeeds(updateData);
        liquidate(vaultID);
    }

    function receiveAuctionProceedings(
        bytes calldata permissionKey
    ) external payable {
        emit VaultReceivedETH(msg.sender, msg.value, permissionKey);
    }

    receive() external payable {}
}

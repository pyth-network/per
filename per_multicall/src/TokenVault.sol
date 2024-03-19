// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./TokenVaultErrors.sol";
import "forge-std/StdMath.sol";
import "./Structs.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";

import "@pythnetwork/pyth-sdk-solidity/PythStructs.sol";
import "@pythnetwork/pyth-sdk-solidity/IPyth.sol";
import "@pythnetwork/express-relay-sdk-solidity/IExpressRelayFeeReceiver.sol";
import "@pythnetwork/express-relay-sdk-solidity/IExpressRelay.sol";

contract TokenVault is IExpressRelayFeeReceiver {
    using SafeERC20 for IERC20;

    event VaultReceivedETH(address sender, uint256 amount, bytes permissionKey);

    uint256 _nVaults;
    address public immutable expressRelay;
    mapping(uint256 => Vault) _vaults;
    address _oracle;

    /**
     * @notice TokenVault constructor - Initializes a new token vault contract with given parameters
     *
     * @param expressRelayAddress: address of the express relay
     * @param oracleAddress: address of the oracle contract
     */
    constructor(address expressRelayAddress, address oracleAddress) {
        _nVaults = 0;
        expressRelay = expressRelayAddress;
        _oracle = oracleAddress;
    }

    /**
     * @notice getLastVaultId function - getter function to get the id of the next vault to be created
     * IDs are sequential and start from 0
     */
    function getLastVaultId() public view returns (uint256) {
        return _nVaults;
    }

    /**
     * @notice convertToUint function - converts a Pyth price struct to a uint256
     *
     * @param price: Pyth price struct to be converted
     * @param targetDecimals: target number of decimals for the output
     */
    function convertToUint(
        PythStructs.Price memory price,
        uint8 targetDecimals
    ) private pure returns (uint256) {
        if (price.price < 0 || price.expo > 0 || price.expo < -255) {
            revert InvalidPriceExponent();
        }

        uint8 priceDecimals = uint8(uint32(-1 * price.expo));

        if (targetDecimals >= priceDecimals) {
            return
                uint(uint64(price.price)) *
                10 ** uint32(targetDecimals - priceDecimals);
        } else {
            return
                uint(uint64(price.price)) /
                10 ** uint32(priceDecimals - targetDecimals);
        }
    }

    /**
     * @notice getPrice function - retrieves price of a given token from the oracle
     *
     * @param id: price feed ID of the token
     */
    function _getPrice(bytes32 id) internal view returns (uint256) {
        IPyth oracle = IPyth(payable(_oracle));
        return convertToUint(oracle.getPrice(id), 18);
    }

    function getOracle() public view returns (address) {
        return _oracle;
    }

    /**
     * @notice getVaultHealth function - calculates vault collateral/debt ratio
     *
     * @param vaultId: ID of the vault for which to calculate health
     */
    function getVaultHealth(uint256 vaultId) public view returns (uint256) {
        Vault memory vault = _vaults[vaultId];
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
        uint256 priceCollateral = _getPrice(vault.tokenIDCollateral);
        uint256 priceDebt = _getPrice(vault.tokenIDDebt);

        if (priceCollateral < 0) {
            revert NegativePrice();
        }
        if (priceDebt < 0) {
            revert NegativePrice();
        }

        uint256 valueCollateral = priceCollateral * vault.amountCollateral;
        uint256 valueDebt = priceDebt * vault.amountDebt;

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
     * @param updateData: data to update price feeds with
     */
    function createVault(
        address tokenCollateral,
        address tokenDebt,
        uint256 amountCollateral,
        uint256 amountDebt,
        uint256 minHealthRatio,
        uint256 minPermissionLessHealthRatio,
        bytes32 tokenIDCollateral,
        bytes32 tokenIDDebt,
        bytes[] calldata updateData
    ) public payable returns (uint256) {
        _updatePriceFeeds(updateData);
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
        if (minPermissionLessHealthRatio > minHealthRatio) {
            revert InvalidHealthRatios();
        }
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
     * @param vaultId: ID of the vault to be updated
     * @param deltaCollateral: delta change to collateral amount (+ means adding collateral tokens, - means removing collateral tokens)
     * @param deltaDebt: delta change to debt amount (+ means withdrawing debt tokens from protocol, - means resending debt tokens to protocol)
     */
    function updateVault(
        uint256 vaultId,
        int256 deltaCollateral,
        int256 deltaDebt
    ) public {
        Vault memory vault = _vaults[vaultId];

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
            _vaults[vaultId].amountCollateral += qCollateral;
        } else {
            // sender takes back collateral from their vault
            IERC20(vault.tokenCollateral).safeTransfer(msg.sender, qCollateral);
            _vaults[vaultId].amountCollateral -= qCollateral;
        }

        // update debt position
        if (deltaDebt >= 0) {
            // sender takes out more debt position
            IERC20(vault.tokenDebt).safeTransfer(msg.sender, qDebt);
            _vaults[vaultId].amountDebt += qDebt;
        } else {
            // sender sends back debt tokens
            IERC20(vault.tokenDebt).safeTransferFrom(
                msg.sender,
                address(this),
                qDebt
            );
            _vaults[vaultId].amountDebt -= qDebt;
        }
    }

    /**
     * @notice getVault function - getter function to get a vault's parameters
     *
     * @param vaultId: ID of the vault
     */
    function getVault(uint256 vaultId) public view returns (Vault memory) {
        return _vaults[vaultId];
    }

    /**
     * @notice _updatePriceFeeds function - updates the specified price feeds with given data
     *
     * @param updateData: data to update price feeds with
     */
    function _updatePriceFeeds(bytes[] calldata updateData) internal {
        if (updateData.length == 0) {
            return;
        }
        IPyth oracle = IPyth(payable(_oracle));
        oracle.updatePriceFeeds{value: msg.value}(updateData);
    }

    /**
     * @notice liquidate function - liquidates a vault
     * This function calculates the health of the vault and based on the vault parameters one of the following actions is taken:
     * 1. If health >= minHealthRatio, don't liquidate
     * 2. If minHealthRatio > health >= minPermissionLessHealthRatio, only liquidate if the vault is permissioned via express relay
     * 3. If minPermissionLessHealthRatio > health, liquidate no matter what
     *
     * @param vaultId: ID of the vault to be liquidated
     */
    function liquidate(uint256 vaultId) public {
        Vault memory vault = _vaults[vaultId];
        uint256 vaultHealth = _getVaultHealth(vault);

        if (vaultHealth >= vault.minHealthRatio) {
            revert InvalidLiquidation();
        }

        if (
            vaultHealth >= vault.minPermissionLessHealthRatio &&
            !IExpressRelay(expressRelay).isPermissioned(
                address(this),
                abi.encode(vaultId)
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

        _vaults[vaultId].amountCollateral = 0;
        _vaults[vaultId].amountDebt = 0;
    }

    /**
     * @notice liquidateWithPriceUpdate function - liquidates a vault after updating the specified price feeds with given data
     *
     * @param vaultId: ID of the vault to be liquidated
     * @param updateData: data to update price feeds with
     */
    function liquidateWithPriceUpdate(
        uint256 vaultId,
        bytes[] calldata updateData
    ) external payable {
        _updatePriceFeeds(updateData);
        liquidate(vaultId);
    }

    function receiveAuctionProceedings(
        bytes calldata permissionKey
    ) external payable {
        emit VaultReceivedETH(msg.sender, msg.value, permissionKey);
    }

    receive() external payable {}
}

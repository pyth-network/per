// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import "./Errors.sol";
import "./Structs.sol";
import "./SigVerify.sol";
import "./ExpressRelay.sol";
import "./WETH9.sol";

import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import "openzeppelin-contracts/contracts/utils/Strings.sol";
import "openzeppelin-contracts-upgradeable/contracts/proxy/utils/Initializable.sol";
import "openzeppelin-contracts-upgradeable/contracts/proxy/utils/UUPSUpgradeable.sol";
import "openzeppelin-contracts-upgradeable/contracts/access/Ownable2StepUpgradeable.sol";
import {ExpressRelay} from "./ExpressRelay.sol";

contract ExpressRelayUpgradable is
    Initializable,
    Ownable2StepUpgradeable,
    UUPSUpgradeable,
    ExpressRelay
{
    event ContractUpgraded(
        address oldImplementation,
        address newImplementation
    );

    // The contract will have an owner and an admin
    // The owner will have all the power over it.
    // The admin can set some config parameters only.
    function initialize(
        address owner,
        address admin,
        address relayer,
        uint256 feeSplitProtocolDefault,
        uint256 feeSplitRelayer
    ) public initializer {
        require(owner != address(0), "owner is zero address");
        require(admin != address(0), "admin is zero address");
        require(relayer != address(0), "relayer is zero address");

        __Ownable_init();
        __UUPSUpgradeable_init();

        ExpressRelay._initialize(
            admin,
            relayer,
            feeSplitProtocolDefault,
            feeSplitRelayer
        );

        // We need to transfer the ownership from deployer to the new owner
        _transferOwnership(owner);
    }

    /// Ensures the contract cannot be uninitialized and taken over.
    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() initializer {}

    // Only allow the owner to upgrade the proxy to a new implementation.
    function _authorizeUpgrade(address) internal override onlyOwner {}

    // We have not overridden these methods in Pyth contracts implementation.
    // But we are overriding them here because there was no owner before and
    // `_authorizeUpgrade` would cause a revert for these. Now we have an owner, and
    // because we want to test for the magic. We are overriding these methods.
    function upgradeTo(address newImplementation) external override onlyProxy {
        address oldImplementation = _getImplementation();
        _authorizeUpgrade(newImplementation);
        _upgradeToAndCallUUPS(newImplementation, new bytes(0), false);

        magicCheck();

        emit ContractUpgraded(oldImplementation, _getImplementation());
    }

    function upgradeToAndCall(
        address newImplementation,
        bytes memory data
    ) external payable override onlyProxy {
        address oldImplementation = _getImplementation();
        _authorizeUpgrade(newImplementation);
        _upgradeToAndCallUUPS(newImplementation, data, true);

        magicCheck();

        emit ContractUpgraded(oldImplementation, _getImplementation());
    }

    function magicCheck() internal view {
        // Calling a method using `this.<method>` will cause a contract call that will use
        // the new contract. This call will fail if the method does not exists or the magic
        // is different.
        if (this.expressRelayUpgradableMagic() != 0x292e6740)
            revert InvalidMagicValue();
    }

    function expressRelayUpgradableMagic() public pure returns (uint32) {
        return 0x292e6740;
    }

    function version() public pure returns (string memory) {
        return "0.1.0";
    }
}

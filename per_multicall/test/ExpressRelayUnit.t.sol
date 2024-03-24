// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import "forge-std/console.sol";

import "../src/Errors.sol";
import "../src/Structs.sol";

import {ExpressRelayTestSetup} from "./ExpressRelayTestSetup.sol";

/**
 * @title ExpressRelayUnitTest
 *
 * ExpressRelayUnitTest is a suite that tests the ExpressRelay contract.
 * This relates to testing the ExpressRelay setter methods and multicall.
 */
contract ExpressRelayUnitTest is Test, ExpressRelayTestSetup {
    function setUp() public {
        setUpWallets();
        setUpContracts();
    }

    function testSetRelayerByAdmin() public {
        address newRelayer = makeAddr("newRelayer");
        vm.prank(admin, admin);
        expressRelay.setRelayer(newRelayer);

        assertEq(expressRelay.getRelayer(), newRelayer);
    }

    function testSetRelayerByNonAdminFail() public {
        address newRelayer = makeAddr("newRelayer");
        vm.expectRevert(abi.encodeWithSelector(Unauthorized.selector));
        vm.prank(relayer, relayer);
        expressRelay.setRelayer(newRelayer);
    }

    function testSetFeeProtocolDefaultByAdmin() public {
        uint256 feeSplitProtocolDefaultPre = expressRelay
            .getFeeProtocolDefault();
        uint256 fee = feeSplitProtocolDefaultPre + 1;
        vm.prank(admin, admin);
        expressRelay.setFeeProtocolDefault(fee);
        uint256 feeSplitProtocolDefaultPost = expressRelay
            .getFeeProtocolDefault();

        assertEq(feeSplitProtocolDefaultPre, feeSplitProtocolDefault);
        assertEq(feeSplitProtocolDefaultPost, feeSplitProtocolDefaultPre + 1);
    }

    function testSetFeeProtocolDefaultByNonAdminFail() public {
        uint256 feeSplitProtocolDefaultPre = expressRelay
            .getFeeProtocolDefault();
        uint256 fee = feeSplitProtocolDefaultPre + 1;
        vm.expectRevert(abi.encodeWithSelector(Unauthorized.selector));
        vm.prank(relayer, relayer);
        expressRelay.setFeeProtocolDefault(fee);
    }

    function testSetFeeProtocolByAdmin() public {
        address protocol = makeAddr("protocol");

        uint256 feeProtocolPre = expressRelay.getFeeProtocol(protocol);
        uint256 fee = feeProtocolPre + 1;
        vm.prank(admin, admin);
        expressRelay.setFeeProtocol(protocol, fee);
        uint256 feeProtocolPost = expressRelay.getFeeProtocol(protocol);

        assertEq(feeProtocolPre, feeSplitProtocolDefault);
        assertEq(feeProtocolPost, feeProtocolPre + 1);
    }

    function testSetFeeProtocolByNonAdminFail() public {
        address protocol = makeAddr("protocol");

        uint256 feeProtocolPre = expressRelay.getFeeProtocol(protocol);
        uint256 fee = feeProtocolPre + 1;
        vm.expectRevert(abi.encodeWithSelector(Unauthorized.selector));
        vm.prank(relayer, relayer);
        expressRelay.setFeeProtocol(protocol, fee);
    }

    function testSetFeeRelayerByAdmin() public {
        uint256 feeSplitRelayerPre = expressRelay.getFeeRelayer();
        uint256 fee = feeSplitRelayerPre + 1;
        vm.prank(admin, admin);
        expressRelay.setFeeRelayer(fee);
        uint256 feeSplitRelayerPost = expressRelay.getFeeRelayer();

        assertEq(feeSplitRelayerPre, feeSplitRelayer);
        assertEq(feeSplitRelayerPost, feeSplitRelayerPre + 1);
    }

    function testSetFeeRelayerByNonAdminFail() public {
        uint256 feeSplitRelayerPre = expressRelay.getFeeRelayer();
        uint256 fee = feeSplitRelayerPre + 1;
        vm.expectRevert(abi.encodeWithSelector(Unauthorized.selector));
        vm.prank(relayer, relayer);
        expressRelay.setFeeRelayer(fee);
    }

    function testMulticallByRelayerEmpty() public {
        bytes memory permission = abi.encode("random permission");
        MulticallData[] memory multicallData;

        vm.prank(relayer, relayer);
        expressRelay.multicall(permission, multicallData);
    }

    function testMulticallByNonRelayerFail() public {
        bytes memory permission = abi.encode("random permission");
        MulticallData[] memory multicallData;

        vm.expectRevert(abi.encodeWithSelector(Unauthorized.selector));
        vm.prank(address(0xbad), address(0xbad));
        expressRelay.multicall(permission, multicallData);
    }
}

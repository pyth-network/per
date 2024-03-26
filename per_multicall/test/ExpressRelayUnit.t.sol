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
        vm.prank(admin);
        expressRelay.setRelayer(newRelayer);

        assertEq(expressRelay.getRelayer(), newRelayer);
    }

    function testSetRelayerByNonAdminFail() public {
        address newRelayer = makeAddr("newRelayer");
        vm.expectRevert(Unauthorized.selector);
        vm.prank(relayer);
        expressRelay.setRelayer(newRelayer);
    }

    function testSetFeeProtocolDefaultByAdmin() public {
        uint256 feeSplitProtocolDefaultPre = expressRelay
            .getFeeProtocolDefault();
        uint256 fee = feeSplitProtocolDefaultPre + 1;
        vm.prank(admin);
        expressRelay.setFeeProtocolDefault(fee);
        uint256 feeSplitProtocolDefaultPost = expressRelay
            .getFeeProtocolDefault();

        assertEq(feeSplitProtocolDefaultPost, feeSplitProtocolDefaultPre + 1);
    }

    function testSetFeeProtocolDefaultByAdminHighFail() public {
        // test setting default fee to the highest valid value
        uint256 feeMax = 10 ** 18;
        vm.prank(admin);
        expressRelay.setFeeProtocolDefault(feeMax);
        uint256 feeProtocolDefaultPost = expressRelay.getFeeProtocolDefault();
        assertEq(feeProtocolDefaultPost, feeMax);

        // test setting default fee to a value higher than the highest valid value, should fail
        uint256 feeInvalid = 10 ** 18 + 1;
        vm.expectRevert(InvalidFeeSplit.selector);
        vm.prank(admin);
        expressRelay.setFeeProtocolDefault(feeInvalid);
    }

    function testSetFeeProtocolDefaultByNonAdminFail() public {
        vm.expectRevert(Unauthorized.selector);
        vm.prank(relayer);
        expressRelay.setFeeProtocolDefault(0);
    }

    function testGetFeeSplitProtocolUncustomized() public {
        address protocol = makeAddr("protocol");
        uint256 feeSplitProtocolDefaultPre = expressRelay
            .getFeeProtocolDefault();
        uint256 feeSplitProtocol = expressRelay.getFeeProtocol(protocol);
        assertEq(feeSplitProtocol, feeSplitProtocolDefaultPre);
    }

    function testSetFeeProtocolByAdmin() public {
        address protocol = makeAddr("protocol");

        uint256 feeProtocolPre = expressRelay.getFeeProtocol(protocol);
        uint256 fee = feeProtocolPre + 1;
        vm.prank(admin);
        expressRelay.setFeeProtocol(protocol, fee);
        uint256 feeProtocolPost = expressRelay.getFeeProtocol(protocol);

        assertEq(feeProtocolPost, feeProtocolPre + 1);
    }

    function testSetFeeProtocolByAdminHighFail() public {
        address protocol = makeAddr("protocol");

        // test setting fee to the highest valid value
        uint256 feeMax = 10 ** 18;
        vm.prank(admin);
        expressRelay.setFeeProtocol(protocol, feeMax);
        uint256 feeProtocolPost = expressRelay.getFeeProtocol(protocol);
        assertEq(feeProtocolPost, feeMax);

        // test setting fee to a value higher than the highest valid value, should fail
        uint256 feeInvalid = 10 ** 18 + 1;
        vm.expectRevert(InvalidFeeSplit.selector);
        vm.prank(admin);
        expressRelay.setFeeProtocol(protocol, feeInvalid);
    }

    function testSetFeeProtocolByNonAdminFail() public {
        address protocol = makeAddr("protocol");

        vm.expectRevert(Unauthorized.selector);
        vm.prank(relayer);
        expressRelay.setFeeProtocol(protocol, 0);
    }

    function testSetFeeRelayerByAdmin() public {
        uint256 feeSplitRelayerPre = expressRelay.getFeeRelayer();
        uint256 fee = feeSplitRelayerPre + 1;
        vm.prank(admin);
        expressRelay.setFeeRelayer(fee);
        uint256 feeSplitRelayerPost = expressRelay.getFeeRelayer();

        assertEq(feeSplitRelayerPre, feeSplitRelayer);
        assertEq(feeSplitRelayerPost, feeSplitRelayerPre + 1);
    }

    function testSetFeeRelayerByAdminHighFail() public {
        // test setting fee to the highest valid value
        uint256 feeMax = 10 ** 18;
        vm.prank(admin);
        expressRelay.setFeeRelayer(feeMax);
        uint256 feeRelayerPost = expressRelay.getFeeRelayer();

        // test setting fee to a value higher than the highest valid value, should fail
        uint256 fee = 10 ** 18 + 1;
        vm.expectRevert(InvalidFeeSplit.selector);
        vm.prank(admin);
        expressRelay.setFeeRelayer(fee);
    }

    function testSetFeeRelayerByNonAdminFail() public {
        vm.expectRevert(Unauthorized.selector);
        vm.prank(relayer);
        expressRelay.setFeeRelayer(0);
    }

    function testMulticallByRelayerEmpty() public {
        bytes memory permission = abi.encode("random permission");
        MulticallData[] memory multicallData;

        vm.prank(relayer);
        expressRelay.multicall(permission, multicallData);
    }

    function testMulticallByNonRelayerFail() public {
        bytes memory permission = abi.encode("random permission");
        MulticallData[] memory multicallData;

        vm.expectRevert(Unauthorized.selector);
        vm.prank(address(0xbad));
        expressRelay.multicall(permission, multicallData);
    }
}

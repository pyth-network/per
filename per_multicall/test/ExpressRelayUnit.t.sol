// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import "forge-std/console.sol";

import "../src/Errors.sol";
import "../src/Structs.sol";

import "@pythnetwork/express-relay-sdk-solidity/IExpressRelayFeeReceiver.sol";
import "@pythnetwork/express-relay-sdk-solidity/IExpressRelay.sol";

import {ExpressRelayTestSetup} from "./ExpressRelayTestSetup.sol";

// Signature: 0x10c280f6
error MockProtocolUnauthorized();

contract MockProtocol is IExpressRelayFeeReceiver {
    address _expressRelay;
    address _feeReceiver;
    event MockProtocolReceivedAuctionProceedings(bytes permissionKey);

    constructor(address expressRelay) {
        _expressRelay = expressRelay;
        _feeReceiver = address(this);
    }

    function setFeeReceiver(address feeReceiver) public {
        _feeReceiver = feeReceiver;
    }

    function execute() public payable {
        if (
            !IExpressRelay(_expressRelay).isPermissioned(
                _feeReceiver,
                abi.encode(uint256(0))
            )
        ) {
            revert MockProtocolUnauthorized();
        }
    }

    function receiveAuctionProceedings(
        bytes calldata permissionKey
    ) external payable {
        emit MockProtocolReceivedAuctionProceedings(permissionKey);
    }
}

contract MockTarget {
    address _expressRelay;
    address _mockProtocol;

    constructor(address expressRelay, address mockProtocol) {
        _expressRelay = expressRelay;
        _mockProtocol = mockProtocol;
    }

    function passThrough(uint256 bid) public payable {
        MockProtocol(_mockProtocol).execute();
        payable(_expressRelay).transfer(bid);
    }
}

/**
 * @title ExpressRelayUnitTest
 *
 * ExpressRelayUnitTest is a suite that tests the ExpressRelay contract.
 * This relates to testing the ExpressRelay setter methods and multicall.
 */
contract ExpressRelayUnitTest is Test, ExpressRelayTestSetup {
    MockProtocol mockProtocol;
    MockTarget mockTarget;

    function setUp() public {
        setUpWallets();
        setUpContracts();

        mockProtocol = new MockProtocol(address(expressRelay));

        mockTarget = new MockTarget(
            address(expressRelay),
            address(mockProtocol)
        );
        vm.deal(address(mockTarget), 1 ether);
    }

    function testSetRelayerByAdmin() public {
        address newRelayer = makeAddr("newRelayer");

        vm.expectEmit(true, true, true, true, address(expressRelay));
        emit RelayerSet(newRelayer);

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

    function testAddRelayerSubwalletByRelayerPrimary() public {
        address subwallet = makeAddr("subwallet");

        vm.expectEmit(true, true, true, true, address(expressRelay));
        emit RelayerSubwalletAdded(relayer, subwallet);

        vm.prank(relayer);
        expressRelay.addRelayerSubwallet(subwallet);
        address[] memory relayerSubwallets = expressRelay
            .getRelayerSubwallets();

        assertAddressInArray(subwallet, relayerSubwallets, true);
    }

    function testAddRelayerSubwalletByNonRelayerPrimaryFail() public {
        address subwallet1 = makeAddr("subwallet1");
        vm.prank(relayer);
        expressRelay.addRelayerSubwallet(subwallet1);

        address subwallet2 = makeAddr("subwallet2");
        vm.expectRevert(Unauthorized.selector);
        vm.prank(subwallet1);
        expressRelay.addRelayerSubwallet(subwallet2);
    }

    function testAddDuplicateRelayerSubwalletByRelayerPrimaryFail() public {
        address subwallet = makeAddr("subwallet");
        vm.prank(relayer);
        expressRelay.addRelayerSubwallet(subwallet);
        vm.expectRevert(DuplicateRelayerSubwallet.selector);
        vm.prank(relayer);
        expressRelay.addRelayerSubwallet(subwallet);
    }

    function testRemoveRelayerSubwalletByRelayerPrimary() public {
        address subwallet1 = makeAddr("subwallet1");
        address subwallet2 = makeAddr("subwallet2");
        vm.prank(relayer);
        expressRelay.addRelayerSubwallet(subwallet1);
        vm.prank(relayer);
        expressRelay.addRelayerSubwallet(subwallet2);
        address[] memory relayerSubwalletsPre = expressRelay
            .getRelayerSubwallets();

        vm.expectEmit(true, true, true, true, address(expressRelay));
        emit RelayerSubwalletRemoved(relayer, subwallet1);

        vm.prank(relayer);
        expressRelay.removeRelayerSubwallet(subwallet1);
        address[] memory relayerSubwalletsPost = expressRelay
            .getRelayerSubwallets();

        assertEq(relayerSubwalletsPre.length, relayerSubwalletsPost.length + 1);
        assertAddressInArray(subwallet1, relayerSubwalletsPost, false);
        assertAddressInArray(subwallet2, relayerSubwalletsPost, true);
    }

    function testRemoveRelayerSubwalletByNonRelayerPrimaryFail() public {
        address subwallet1 = makeAddr("subwallet1");
        address subwallet2 = makeAddr("subwallet2");
        vm.prank(relayer);
        expressRelay.addRelayerSubwallet(subwallet1);
        vm.prank(relayer);
        expressRelay.addRelayerSubwallet(subwallet2);

        vm.expectRevert(Unauthorized.selector);
        vm.prank(subwallet1);
        expressRelay.removeRelayerSubwallet(subwallet2);
    }

    function testRemoveNonExistentRelayerSubwalletByRelayerFail() public {
        address subwallet = makeAddr("subwallet");
        vm.prank(relayer);
        expressRelay.addRelayerSubwallet(subwallet);

        address nonExistentSubwallet = makeAddr("nonExistentSubwallet");
        vm.expectRevert(RelayerSubwalletNotFound.selector);
        vm.prank(relayer);
        expressRelay.removeRelayerSubwallet(nonExistentSubwallet);
    }

    function testChangeRelayerAfterAddingRelayerSubwallet() public {
        address subwallet = makeAddr("subwallet");
        vm.prank(relayer);
        expressRelay.addRelayerSubwallet(subwallet);
        address[] memory expectedSubwallets = new address[](1);
        expectedSubwallets[0] = subwallet;
        assertEq(expressRelay.getRelayerSubwallets(), expectedSubwallets);

        address newRelayer = makeAddr("newRelayer");

        vm.expectEmit(true, true, true, true, address(expressRelay));
        emit RelayerSet(newRelayer);

        vm.prank(admin);
        expressRelay.setRelayer(newRelayer);

        assertEq(expressRelay.getRelayer(), newRelayer);
        assertEq(expressRelay.getRelayerSubwallets(), new address[](0));
    }

    function testSetFeeProtocolDefaultByAdmin() public {
        uint256 feeSplitProtocolDefaultPre = expressRelay
            .getFeeProtocolDefault();
        uint256 fee = feeSplitProtocolDefaultPre + 1;

        vm.expectEmit(true, true, true, true, address(expressRelay));
        emit FeeProtocolDefaultSet(fee);

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

        vm.expectEmit(true, true, true, true, address(expressRelay));
        emit FeeProtocolSet(protocol, fee);

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

        vm.expectEmit(true, true, true, true, address(expressRelay));
        emit FeeRelayerSet(fee);

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

    function testMulticallByRelayer() public {
        (, , bytes memory permission) = generateRandomPermission();
        MulticallData[] memory multicallData;

        vm.prank(relayer);
        expressRelay.multicall(permission, multicallData);
    }

    function testMulticallByRelayerSubwallet() public {
        (, , bytes memory permission) = generateRandomPermission();

        address subwallet = makeAddr("subwallet");
        vm.prank(relayer);
        expressRelay.addRelayerSubwallet(subwallet);

        MulticallData[] memory multicallData;

        vm.prank(subwallet);
        expressRelay.multicall(permission, multicallData);
    }

    function testMulticallByNonRelayer() public {
        (, , bytes memory permission) = generateRandomPermission();

        MulticallData[] memory multicallData;

        vm.expectRevert(Unauthorized.selector);
        vm.prank(address(0xbad));
        expressRelay.multicall(permission, multicallData);
    }

    function testMulticallPermissionToggle() public {
        (
            address protocol,
            bytes memory permissionId,
            bytes memory permission
        ) = generateRandomPermission();

        assert(!expressRelay.isPermissioned(protocol, permissionId));

        MulticallData[] memory multicallData;
        vm.prank(relayer);
        expressRelay.multicall(permission, multicallData);

        assert(!expressRelay.isPermissioned(protocol, permissionId));
    }

    function testMulticallInvalidPermissionFail() public {
        bytes memory permission = abi.encodePacked(uint8(0));
        MulticallData[] memory multicallData;

        vm.expectRevert(InvalidPermission.selector);
        vm.prank(relayer);
        expressRelay.multicall(permission, multicallData);
    }

    function testMulticallMockTarget() public {
        address feeReceiver = address(mockProtocol);
        BalancesMockTarget memory balancesPre = getBalancesMockTarget(
            feeReceiver,
            address(mockTarget)
        );

        bytes memory permission = abi.encode(
            address(feeReceiver),
            abi.encode(uint256(0))
        );

        uint256 bid = 100;
        bytes memory targetCalldata = abi.encodeWithSelector(
            mockTarget.passThrough.selector,
            bid
        );

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(mockTarget);
        data[0] = targetCalldata;
        bidInfos[0] = makeBidInfo(bid, searcherAOwnerSk);

        MulticallData[] memory multicallData = getMulticallData(
            contracts,
            data,
            bidInfos
        );

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](1);
        expectedMulticallStatuses[0].externalSuccess = true;
        expectMulticallIssued(
            permission,
            multicallData,
            expectedMulticallStatuses
        );

        vm.prank(relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            multicallData
        );

        BalancesMockTarget memory balancesPost = getBalancesMockTarget(
            feeReceiver,
            address(mockTarget)
        );

        checkMulticallStatuses(
            multicallStatuses,
            expectedMulticallStatuses,
            true,
            false,
            true
        );

        assertExpectedBidPaymentMockTarget(
            balancesPre,
            balancesPost,
            bidInfos,
            multicallStatuses
        );
    }

    // function testMulticallMockTargetFail()
    // function testMulticallMockTargetEoaFeeReceiver()
    // function testMulticallMockTargetWrongPermissionFail()
    // function testMulticallMockTargetWrongMismatchedBidFail()
    // function testMulticallMockTargetMultiple()
    // function testMulticallMockTargetMultipleFailSecond()--use invalid data/selector
    // function

    // function testCallWithBidByContract() public {

    // }
}

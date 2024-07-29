// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";

import "src/express-relay/Errors.sol";
import "src/express-relay/Structs.sol";

import {ExpressRelayTestSetup} from "./ExpressRelayTestSetup.sol";
import "./helpers/MockProtocol.sol";
import {GasVerifier} from "./helpers/MulticallHelpers.sol";

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

        setUpExpressRelayHarness();

        mockProtocol = new MockProtocol(address(expressRelay));

        mockTarget = new MockTarget(
            address(expressRelay),
            address(mockProtocol)
        );
        vm.deal(address(mockTarget), 1 ether);
    }

    function testGetAdmin() public {
        assertEq(expressRelay.getAdmin(), admin);
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
        vm.prank(subwallet2);
        expressRelay.removeRelayerSubwallet(subwallet1);
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

    function testSetFeeSplitPrecision() public {
        expressRelayHarness.exposed_setFeeSplitPrecision();
        uint256 feeSplitPrecision = expressRelayHarness.getFeeSplitPrecision();
        assertEq(feeSplitPrecision, 10 ** 18);
    }

    function testValidateFeeSplit(uint256 feeSplit) public {
        uint256 feeSplitPrecision = expressRelay.getFeeSplitPrecision();
        if (feeSplit > feeSplitPrecision) {
            vm.expectRevert(InvalidFeeSplit.selector);
        }
        expressRelayHarness.exposed_validateFeeSplit(feeSplit);
    }

    function testValidateFeeSplitMax() public view {
        uint256 feeSplit = expressRelay.getFeeSplitPrecision();
        expressRelayHarness.exposed_validateFeeSplit(feeSplit);
    }

    function testIsContract() public view {
        assert(expressRelayHarness.exposed_isContract(address(this)));
        assert(expressRelayHarness.exposed_isContract(address(expressRelay)));
        assert(expressRelayHarness.exposed_isContract(address(mockProtocol)));
        assert(expressRelayHarness.exposed_isContract(address(mockTarget)));
        assert(expressRelayHarness.exposed_isContract(address(adapterFactory)));
        assert(expressRelayHarness.exposed_isContract(address(tokenVault)));
        assert(expressRelayHarness.exposed_isContract(address(weth)));
        assert(expressRelayHarness.exposed_isContract(address(searcherA)));
        assert(expressRelayHarness.exposed_isContract(address(searcherB)));

        assert(!expressRelayHarness.exposed_isContract(address(0)));
        assert(!expressRelayHarness.exposed_isContract(address(0xdeadbeef)));
        assert(!expressRelayHarness.exposed_isContract(relayer));
        assert(!expressRelayHarness.exposed_isContract(admin));
        assert(!expressRelayHarness.exposed_isContract(searcherAOwnerAddress));
        assert(!expressRelayHarness.exposed_isContract(searcherBOwnerAddress));
        assert(!expressRelayHarness.exposed_isContract(tokenVaultDeployer));
        assert(!expressRelayHarness.exposed_isContract(depositor));
    }

    function testBytesToAddress(address addr, bytes memory data) public {
        bytes memory addrBytes = abi.encode(addr, data);
        address addrDecoded = expressRelayHarness.exposed_bytesToAddress(
            addrBytes
        );
        assertEq(addrDecoded, addr);
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
        // permission is 20 bytes, so this should not trigger invalid permission error
        bytes memory permissionValid = abi.encodePacked(uint160(0));
        MulticallData[] memory multicallData;

        vm.prank(relayer);
        expressRelay.multicall(permissionValid, multicallData);

        // permission is 19 bytes, so this should trigger invalid permission error
        bytes memory permissionInvalid = abi.encodePacked(uint152(0));

        vm.expectRevert(InvalidPermission.selector);
        vm.prank(relayer);
        expressRelay.multicall(permissionInvalid, multicallData);
    }

    function testMulticallMockTarget() public {
        address feeReceiver = address(mockProtocol);

        uint256 bid = 100;

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(mockTarget);
        data[0] = abi.encodeWithSelector(mockTarget.passThrough.selector, bid);
        bidInfos[0] = makeBidInfo(bid, searcherAOwnerSk);

        (
            bytes memory permission,
            MulticallData[] memory multicallData
        ) = makeMulticallMockTargetCall(feeReceiver, contracts, data, bidInfos);

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](1);
        expectedMulticallStatuses[0].externalSuccess = true;

        uint256[] memory bidsExpectedSuccessful = new uint256[](1);
        bidsExpectedSuccessful[0] = bid;

        BalancesMockTarget memory balancesPre = getBalancesMockTarget(
            feeReceiver,
            address(mockTarget)
        );
        BalancesMockTarget
            memory balancesPostExpected = getExpectedPostBidBalances(
                balancesPre,
                bidsExpectedSuccessful,
                feeReceiver
            );

        runMulticallMockTargetSuccessfulAndCheck(
            permission,
            multicallData,
            expectedMulticallStatuses,
            address(mockTarget),
            balancesPostExpected
        );
    }

    function testMulticallMockTargetFail() public {
        address feeReceiver = address(mockProtocol);

        uint256 bid = 100;

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(mockTarget);
        // use the failing function, bid should not be paid out
        data[0] = abi.encodeWithSelector(
            mockTarget.passThroughFail.selector,
            bid
        );
        bidInfos[0] = makeBidInfo(bid, searcherAOwnerSk);

        (
            bytes memory permission,
            MulticallData[] memory multicallData
        ) = makeMulticallMockTargetCall(feeReceiver, contracts, data, bidInfos);

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](1);
        expectedMulticallStatuses[0].externalResult = abi.encodeWithSelector(
            MockProtocolFail.selector
        );

        BalancesMockTarget memory balancesPre = getBalancesMockTarget(
            feeReceiver,
            address(mockTarget)
        );

        runMulticallMockTargetSuccessfulAndCheck(
            permission,
            multicallData,
            expectedMulticallStatuses,
            address(mockTarget),
            balancesPre
        );
    }

    function testMulticallMockTargetEoaFeeReceiver() public {
        address feeReceiver = makeAddr("feeReceiverMockProtocol");
        mockProtocol.setFeeReceiver(feeReceiver);

        uint256 bid = 100;

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(mockTarget);
        data[0] = abi.encodeWithSelector(mockTarget.passThrough.selector, bid);
        bidInfos[0] = makeBidInfo(bid, searcherAOwnerSk);

        (
            bytes memory permission,
            MulticallData[] memory multicallData
        ) = makeMulticallMockTargetCall(feeReceiver, contracts, data, bidInfos);

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](1);
        expectedMulticallStatuses[0].externalSuccess = true;

        uint256[] memory bidsExpectedSuccessful = new uint256[](1);
        bidsExpectedSuccessful[0] = bid;

        BalancesMockTarget memory balancesPre = getBalancesMockTarget(
            feeReceiver,
            address(mockTarget)
        );
        BalancesMockTarget
            memory balancesPostExpected = getExpectedPostBidBalances(
                balancesPre,
                bidsExpectedSuccessful,
                feeReceiver
            );

        runMulticallMockTargetSuccessfulAndCheck(
            permission,
            multicallData,
            expectedMulticallStatuses,
            address(mockTarget),
            balancesPostExpected
        );
    }

    function testMulticallMockTargetWrongPermissionFail() public {
        address feeReceiver = address(mockProtocol);

        uint256 bid = 100;

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(mockTarget);
        data[0] = abi.encodeWithSelector(mockTarget.passThrough.selector, bid);
        bidInfos[0] = makeBidInfo(bid, searcherAOwnerSk);

        (
            bytes memory permission,
            MulticallData[] memory multicallData
        ) = makeMulticallMockTargetCall(feeReceiver, contracts, data, bidInfos);

        // intentionally use incorrect permission
        permission = abi.encodePacked(address(feeReceiver), uint256(1));

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](1);
        expectedMulticallStatuses[0].externalResult = abi.encodeWithSelector(
            MockProtocolUnauthorized.selector
        );

        BalancesMockTarget memory balancesPre = getBalancesMockTarget(
            feeReceiver,
            address(mockTarget)
        );

        runMulticallMockTargetSuccessfulAndCheck(
            permission,
            multicallData,
            expectedMulticallStatuses,
            address(mockTarget),
            balancesPre
        );
    }

    function testMulticallMockTargetWrongMismatchedBidFail() public {
        address feeReceiver = address(mockProtocol);

        // use different amounts in the asserted bid and the actual payment
        uint256 bidAsserted = 100;
        uint256 bidPaid = 90;

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(mockTarget);
        data[0] = abi.encodeWithSelector(
            mockTarget.passThrough.selector,
            bidPaid
        );
        bidInfos[0] = makeBidInfo(bidAsserted, searcherAOwnerSk);

        (
            bytes memory permission,
            MulticallData[] memory multicallData
        ) = makeMulticallMockTargetCall(feeReceiver, contracts, data, bidInfos);

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](1);
        expectedMulticallStatuses[0].externalSuccess = false;
        expectedMulticallStatuses[0].multicallRevertReason = "invalid bid";

        BalancesMockTarget memory balancesPre = getBalancesMockTarget(
            feeReceiver,
            address(mockTarget)
        );

        runMulticallMockTargetSuccessfulAndCheck(
            permission,
            multicallData,
            expectedMulticallStatuses,
            address(mockTarget),
            balancesPre
        );
    }

    function testMulticallMockTargetMultiple() public {
        address feeReceiver = address(mockProtocol);

        uint256 bid0 = 100;
        uint256 bid1 = 93;

        address[] memory contracts = new address[](2);
        bytes[] memory data = new bytes[](2);
        BidInfo[] memory bidInfos = new BidInfo[](2);

        contracts[0] = address(mockTarget);
        data[0] = abi.encodeWithSelector(mockTarget.passThrough.selector, bid0);
        bidInfos[0] = makeBidInfo(bid0, searcherAOwnerSk);

        contracts[1] = address(mockTarget);
        data[1] = abi.encodeWithSelector(mockTarget.passThrough.selector, bid1);
        bidInfos[1] = makeBidInfo(bid1, searcherBOwnerSk);

        (
            bytes memory permission,
            MulticallData[] memory multicallData
        ) = makeMulticallMockTargetCall(feeReceiver, contracts, data, bidInfos);

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](2);
        expectedMulticallStatuses[0].externalSuccess = true;
        expectedMulticallStatuses[1].externalSuccess = true;

        uint256[] memory bidsExpectedSuccessful = new uint256[](2);
        bidsExpectedSuccessful[0] = bid0;
        bidsExpectedSuccessful[1] = bid1;

        BalancesMockTarget memory balancesPre = getBalancesMockTarget(
            feeReceiver,
            address(mockTarget)
        );
        BalancesMockTarget
            memory balancesPostExpected = getExpectedPostBidBalances(
                balancesPre,
                bidsExpectedSuccessful,
                feeReceiver
            );

        runMulticallMockTargetSuccessfulAndCheck(
            permission,
            multicallData,
            expectedMulticallStatuses,
            address(mockTarget),
            balancesPostExpected
        );
    }

    function testMulticallMockTargetMultipleFailSecond() public {
        address feeReceiver = address(mockProtocol);

        uint256 bid0 = 100;
        uint256 bid1 = 93;

        address[] memory contracts = new address[](2);
        bytes[] memory data = new bytes[](2);
        BidInfo[] memory bidInfos = new BidInfo[](2);

        contracts[0] = address(mockTarget);
        data[0] = abi.encodeWithSelector(mockTarget.passThrough.selector, bid0);
        bidInfos[0] = makeBidInfo(bid0, searcherAOwnerSk);

        contracts[1] = address(mockTarget);
        // use the failing function, bid1 should not be paid out
        data[1] = abi.encodeWithSelector(
            mockTarget.passThroughFail.selector,
            bid1
        );
        bidInfos[1] = makeBidInfo(bid1, searcherBOwnerSk);

        (
            bytes memory permission,
            MulticallData[] memory multicallData
        ) = makeMulticallMockTargetCall(feeReceiver, contracts, data, bidInfos);

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](2);
        expectedMulticallStatuses[0].externalSuccess = true;
        expectedMulticallStatuses[1].externalResult = abi.encodeWithSelector(
            MockProtocolFail.selector
        );

        uint256[] memory bidsExpectedSuccessful = new uint256[](1);
        bidsExpectedSuccessful[0] = bid0;

        BalancesMockTarget memory balancesPre = getBalancesMockTarget(
            feeReceiver,
            address(mockTarget)
        );
        BalancesMockTarget
            memory balancesPostExpected = getExpectedPostBidBalances(
                balancesPre,
                bidsExpectedSuccessful,
                feeReceiver
            );

        runMulticallMockTargetSuccessfulAndCheck(
            permission,
            multicallData,
            expectedMulticallStatuses,
            address(mockTarget),
            balancesPostExpected
        );
    }

    function testMulticallMockTargetInvalidDataFail() public {
        address feeReceiver = address(mockProtocol);

        uint256 bid = 100;

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(mockTarget);
        // use invalid data, should fail
        data[0] = abi.encodeWithSelector(bytes4(0xDEADBEEF));
        bidInfos[0] = makeBidInfo(bid, searcherAOwnerSk);

        (
            bytes memory permission,
            MulticallData[] memory multicallData
        ) = makeMulticallMockTargetCall(feeReceiver, contracts, data, bidInfos);

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](1);

        BalancesMockTarget memory balancesPre = getBalancesMockTarget(
            feeReceiver,
            address(mockTarget)
        );

        runMulticallMockTargetSuccessfulAndCheck(
            permission,
            multicallData,
            expectedMulticallStatuses,
            address(mockTarget),
            balancesPre
        );
    }

    function testCallWithBidByContractFail() public {
        address feeReceiver = address(mockProtocol);

        uint256 bid = 100;

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(mockTarget);
        data[0] = abi.encodeWithSelector(mockTarget.passThrough.selector, bid);
        bidInfos[0] = makeBidInfo(bid, searcherAOwnerSk);

        (, MulticallData[] memory multicallData) = makeMulticallMockTargetCall(
            feeReceiver,
            contracts,
            data,
            bidInfos
        );

        BalancesMockTarget memory balancesPre = getBalancesMockTarget(
            feeReceiver,
            address(mockTarget)
        );

        vm.prank(address(expressRelay));
        (bool success, bytes memory result) = expressRelay.callWithBid(
            multicallData[0]
        );

        BalancesMockTarget memory balancesPost = getBalancesMockTarget(
            feeReceiver,
            address(mockTarget)
        );

        assert(!success);
        // should fail bc permission isn't turned on
        assertEq(bytes4(result), MockProtocolUnauthorized.selector);

        assertEqBalancesMockTarget(balancesPost, balancesPre);
    }

    function testCallWithBidByNonContractFail(address caller) public {
        vm.assume(caller != address(expressRelay));

        address feeReceiver = address(mockProtocol);

        uint256 bid = 100;

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(mockTarget);
        data[0] = abi.encodeWithSelector(mockTarget.passThrough.selector, bid);
        bidInfos[0] = makeBidInfo(bid, searcherAOwnerSk);

        (, MulticallData[] memory multicallData) = makeMulticallMockTargetCall(
            feeReceiver,
            contracts,
            data,
            bidInfos
        );

        vm.expectRevert(Unauthorized.selector);
        vm.prank(caller);
        expressRelay.callWithBid(multicallData[0]);
    }

    function testCallWithBidByContractInvalidDataFail() public {
        address feeReceiver = address(mockProtocol);

        uint256 bid = 100;

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(mockTarget);
        data[0] = abi.encodeWithSelector(bytes4(0xDEADBEEF));
        bidInfos[0] = makeBidInfo(bid, searcherAOwnerSk);

        (, MulticallData[] memory multicallData) = makeMulticallMockTargetCall(
            feeReceiver,
            contracts,
            data,
            bidInfos
        );

        BalancesMockTarget memory balancesPre = getBalancesMockTarget(
            feeReceiver,
            address(mockTarget)
        );

        vm.prank(address(expressRelay));
        (bool success, ) = expressRelay.callWithBid(multicallData[0]);

        BalancesMockTarget memory balancesPost = getBalancesMockTarget(
            feeReceiver,
            address(mockTarget)
        );

        assert(!success);

        assertEqBalancesMockTarget(balancesPost, balancesPre);
    }

    function testCallWithBidByContractPermissionless() public {
        address feeReceiver = address(mockProtocol);

        uint256 bid = 100;

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(mockTarget);
        data[0] = abi.encodeWithSelector(
            mockTarget.passThroughPermissionless.selector,
            bid
        );
        bidInfos[0] = makeBidInfo(bid, searcherAOwnerSk);

        (, MulticallData[] memory multicallData) = makeMulticallMockTargetCall(
            feeReceiver,
            contracts,
            data,
            bidInfos
        );

        BalancesMockTarget memory balancesPre = getBalancesMockTarget(
            feeReceiver,
            address(mockTarget)
        );

        vm.prank(address(expressRelay));
        (bool success, ) = expressRelay.callWithBid(multicallData[0]);

        assert(success);

        BalancesMockTarget memory balancesPost = getBalancesMockTarget(
            feeReceiver,
            address(mockTarget)
        );

        // no payout of the bids to relayer or protocol within callWithBid
        assertEq(
            balancesPost.balanceExpressRelay,
            balancesPre.balanceExpressRelay + bid
        );
        assertEq(
            balancesPost.balanceMockTarget,
            balancesPre.balanceMockTarget - bid
        );
    }

    function testCallWithBidByContractPermissionlessMismatchedBidFail() public {
        address feeReceiver = address(mockProtocol);

        uint256 bidAsserted = 100;
        uint256 bidPaid = 93;

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(mockTarget);
        data[0] = abi.encodeWithSelector(
            mockTarget.passThroughPermissionless.selector,
            bidPaid
        );
        bidInfos[0] = makeBidInfo(bidAsserted, searcherAOwnerSk);

        (, MulticallData[] memory multicallData) = makeMulticallMockTargetCall(
            feeReceiver,
            contracts,
            data,
            bidInfos
        );

        vm.expectRevert("invalid bid");
        vm.prank(address(expressRelay));
        expressRelay.callWithBid(multicallData[0]);
    }

    function testCallWithBidCallSelfFail() public {
        address feeReceiver = address(mockProtocol);

        uint256 bid = 100;

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(expressRelay);
        bidInfos[0] = makeBidInfo(bid, searcherAOwnerSk);

        (, MulticallData[] memory multicallData) = makeMulticallMockTargetCall(
            feeReceiver,
            contracts,
            data,
            bidInfos
        );

        vm.expectRevert(InvalidTargetContract.selector);
        vm.prank(address(expressRelay));
        expressRelay.callWithBid(multicallData[0]);
    }

    function testCallGasLimit() public {
        bytes memory permission = abi.encode(address(this), "0");
        GasVerifier verifier = new GasVerifier();
        MulticallData[] memory multicallData = new MulticallData[](1);
        multicallData[0] = MulticallData(
            "1",
            address(verifier),
            abi.encodeWithSelector(verifier.verifyGas.selector),
            0,
            1000,
            false
        );
        vm.prank(relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            multicallData
        );
        assertEq(multicallStatuses[0].externalSuccess, true);

        multicallData[0].gasLimit = 1150;
        vm.prank(relayer);
        multicallStatuses = expressRelay.multicall(permission, multicallData);
        assertEq(multicallStatuses[0].externalSuccess, false);
    }
}

// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import "forge-std/console.sol";

import "../src/Structs.sol";
import "../src/Errors.sol";
import "../src/TokenVaultErrors.sol";

import {ExpressRelayTestSetup} from "./ExpressRelayTestSetup.sol";

/**
 * @title ExpressRelayIntegrationTest
 *
 * ExpressRelayIntegrationTest is a suite that tests the integration of the various contracts in the ExpressRelay stack.
 * This includes the ExpressRelay entrypoint contract for all relay interactions, the TokenVault dummy lending protocol contract, individual searcher contracts programmed to perform liquidations, the OpportunityAdapter contract used to facilitate liquidations directly from searcher EOAs, and the relevant token contracts.
 * We test the integration of these contracts by creating vaults in the TokenVault protocol, simulating undercollateralization of these vaults to trigger liquidations, constructing the necessary liquidation data, and then calling liquidation through OpportunityAdapter or the searcher contracts.
 *
 * The focus in these tests is ensuring that liquidation succeeds (or fails as expected) through the ExpressRelay contrct routing to the searcher contracts or the OpportunityAdapter contract.
 */
contract ExpressRelayIntegrationTest is Test, ExpressRelayTestSetup {
    /**
     * @notice setUp function - sets up the contracts, wallets, tokens, oracle feeds, and vaults for the test
     */
    function setUp() public {
        setUpWallets();
        setUpContracts();
        setUpTokensAndOracle();
        setUpVaults();
        fundSearcherWallets();
    }

    function testLiquidateNoPER() public {
        uint vaultNumber = 0;
        // test permissionless liquidation (success)
        // raise price of debt token to make vault 0 undercollateralized
        bytes memory tokenDebtUpdateData = createPriceFeedUpdateSimple(
            mockPyth,
            idsDebt[vaultNumber],
            tokenDebtPricesLiqPermissionless[vaultNumber],
            tokenExpo
        );

        bytes memory signatureSearcher;

        uint256 validUntil = UINT256_MAX;

        AccountBalance memory balancesAPre = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        vm.prank(searcherAOwnerAddress);
        searcherA.doLiquidate(
            0,
            0,
            validUntil,
            tokenDebtUpdateData,
            signatureSearcher
        );

        AccountBalance memory balancesAPost = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        assertEq(
            balancesAPost.collateral,
            balancesAPre.collateral + amountsCollateral[vaultNumber]
        );
        assertEq(
            balancesAPost.debt,
            balancesAPre.debt - amountsDebt[vaultNumber]
        );
    }

    function testLiquidateNoPERFail() public {
        uint vaultNumber = 0;
        // test permissionless liquidation (failure)
        // raise price of debt token to make vault 0 undercollateralized
        bytes memory tokenDebtUpdateData = createPriceFeedUpdateSimple(
            mockPyth,
            idsDebt[vaultNumber],
            tokenDebtPricesLiqExpressRelay[vaultNumber],
            tokenExpo
        );

        bytes memory signatureSearcher;

        uint256 validUntil = UINT256_MAX;

        vm.expectRevert(InvalidLiquidation.selector);
        vm.prank(searcherAOwnerAddress);
        searcherA.doLiquidate(
            0,
            0,
            validUntil,
            tokenDebtUpdateData,
            signatureSearcher
        );
    }

    function testLiquidateSingle() public {
        // test ExpressRelay path liquidation (via multicall, express relay operator calls) with searcher contract
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        uint256 bidAmount0 = 150;
        contracts[0] = address(searcherA);
        bidInfos[0] = makeBidInfo(bidAmount0, searcherAOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoSearcherContracts(vaultNumber, contracts, bidInfos);

        MulticallData[] memory multicallData = getMulticallData(
            contracts,
            data,
            bidInfos
        );

        uint256 balanceProtocolPre = address(tokenVault).balance;
        AccountBalance memory balancesAPre = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](1);
        expectedMulticallStatuses[0].externalSuccess = true;
        expectMulticallIssuedEmit(
            permission,
            multicallData,
            expectedMulticallStatuses
        );

        vm.prank(relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            multicallData
        );

        uint256 balanceProtocolPost = address(tokenVault).balance;
        AccountBalance memory balancesAPost = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        assertEq(
            balancesAPost.collateral,
            balancesAPre.collateral + amountsCollateral[vaultNumber]
        );
        assertEq(
            balancesAPost.debt,
            balancesAPre.debt - amountsDebt[vaultNumber]
        );

        checkMulticallStatuses(
            multicallStatuses,
            expectedMulticallStatuses,
            true
        );

        uint256 feeSplitProtocol = expressRelay.getFeeProtocol(
            address(tokenVault)
        );
        uint256 feeSplitPrecision = expressRelay.getFeeSplitPrecision();
        assertEq(
            balanceProtocolPost,
            balanceProtocolPre +
                (bidAmount0 * feeSplitProtocol) /
                feeSplitPrecision
        );
    }

    /**
     * @notice Test a multicall with two calls, where the second is expected to fail
     *
     * The first call should succeed and liquidate the vault. The second should therefore fail, bc the vault is already liquidated.
     */
    function testLiquidateMultipleFailSecond() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](2);
        BidInfo[] memory bidInfos = new BidInfo[](2);

        uint256 bidAmount0 = 150;
        contracts[0] = address(searcherA);
        bidInfos[0] = makeBidInfo(bidAmount0, searcherAOwnerSk);

        uint256 bidAmount1 = 100;
        contracts[1] = address(searcherB);
        bidInfos[1] = makeBidInfo(bidAmount1, searcherAOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoSearcherContracts(vaultNumber, contracts, bidInfos);

        MulticallData[] memory multicallData = getMulticallData(
            contracts,
            data,
            bidInfos
        );

        uint256 balanceProtocolPre = address(tokenVault).balance;
        AccountBalance memory balancesAPre = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );
        AccountBalance memory balancesBPre = getBalances(
            address(searcherB),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](2);
        expectedMulticallStatuses[0].externalSuccess = true;
        expectedMulticallStatuses[1].externalSuccess = false;
        expectMulticallIssuedEmit(
            permission,
            multicallData,
            expectedMulticallStatuses
        );

        vm.prank(relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            multicallData
        );
        uint256 balanceProtocolPost = address(tokenVault).balance;
        AccountBalance memory balancesAPost = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        AccountBalance memory balancesBPost = getBalances(
            address(searcherB),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        assertEq(
            balancesAPost.collateral,
            balancesAPre.collateral + amountsCollateral[vaultNumber]
        );
        assertEq(
            balancesAPost.debt,
            balancesAPre.debt - amountsDebt[vaultNumber]
        );

        assertEq(balancesBPost.collateral, balancesBPre.collateral);
        assertEq(balancesBPost.debt, balancesBPre.debt);

        logMulticallStatuses(multicallStatuses);

        // set checkExternalResult to false, failure comes from an unknown error
        checkMulticallStatuses(
            multicallStatuses,
            expectedMulticallStatuses,
            false
        );

        uint256 feeSplitProtocol = expressRelay.getFeeProtocol(
            address(tokenVault)
        );
        uint256 feeSplitPrecision = expressRelay.getFeeSplitPrecision();
        assertEq(
            balanceProtocolPost,
            balanceProtocolPre +
                (bidAmount0 * feeSplitProtocol) /
                feeSplitPrecision
        );
    }

    /**
     * @notice Test a multicall with two calls, where the first is expected to fail
     *
     * The first call should fail, bc the searcher contract has no Eth to pay the express relay. The second should therefore succeed in liquidating the vault.
     */
    function testLiquidateMultipleFailFirst() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](2);
        BidInfo[] memory bidInfos = new BidInfo[](2);

        uint256 bidAmount0 = 150;
        contracts[0] = address(searcherA);
        bidInfos[0] = makeBidInfo(bidAmount0, searcherAOwnerSk);

        uint256 bidAmount1 = 100;
        contracts[1] = address(searcherB);
        bidInfos[1] = makeBidInfo(bidAmount1, searcherBOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoSearcherContracts(vaultNumber, contracts, bidInfos);

        MulticallData[] memory multicallData = getMulticallData(
            contracts,
            data,
            bidInfos
        );

        uint256 balanceProtocolPre = address(tokenVault).balance;
        AccountBalance memory balancesAPre = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );
        AccountBalance memory balancesBPre = getBalances(
            address(searcherB),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        // drain searcherA contract of Eth, so that the first liquidation fails
        vm.prank(searcherAOwnerAddress);
        searcherA.withdrawEth(address(searcherA).balance);

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](2);
        expectedMulticallStatuses[0].externalSuccess = false;
        expectedMulticallStatuses[1].externalSuccess = true;
        expectMulticallIssuedEmit(
            permission,
            multicallData,
            expectedMulticallStatuses
        );

        vm.prank(relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            multicallData
        );

        uint256 balanceProtocolPost = address(tokenVault).balance;

        AccountBalance memory balancesAPost = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );
        AccountBalance memory balancesBPost = getBalances(
            address(searcherB),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        assertEq(balancesAPost.collateral, balancesAPre.collateral);
        assertEq(balancesAPost.debt, balancesAPre.debt);

        assertEq(
            balancesBPost.collateral,
            balancesBPre.collateral + amountsCollateral[vaultNumber]
        );
        assertEq(
            balancesBPost.debt,
            balancesBPre.debt - amountsDebt[vaultNumber]
        );

        logMulticallStatuses(multicallStatuses);

        checkMulticallStatuses(
            multicallStatuses,
            expectedMulticallStatuses,
            true
        );

        uint256 feeSplitProtocol = expressRelay.getFeeProtocol(
            address(tokenVault)
        );
        uint256 feeSplitPrecision = expressRelay.getFeeSplitPrecision();
        assertEq(
            balanceProtocolPost,
            balanceProtocolPre +
                (bidAmount1 * feeSplitProtocol) /
                feeSplitPrecision
        );
    }

    function testLiquidateWrongPermissionFail() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        uint256 bidAmount0 = 150;
        contracts[0] = address(searcherA);
        bidInfos[0] = makeBidInfo(bidAmount0, searcherAOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoSearcherContracts(vaultNumber, contracts, bidInfos);

        MulticallData[] memory multicallData = getMulticallData(
            contracts,
            data,
            bidInfos
        );

        // wrong permisison key
        permission = abi.encode(address(0));

        uint256 balanceProtocolPre = address(tokenVault).balance;
        AccountBalance memory balancesAPre = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](1);
        expectedMulticallStatuses[0].externalResult = abi.encodeWithSelector(
            InvalidLiquidation.selector
        );
        expectMulticallIssuedEmit(
            permission,
            multicallData,
            expectedMulticallStatuses
        );

        vm.prank(relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            multicallData
        );

        uint256 balanceProtocolPost = address(tokenVault).balance;
        AccountBalance memory balancesAPost = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        assertEq(balancesAPost.collateral, balancesAPre.collateral);
        assertEq(balancesAPost.debt, balancesAPre.debt);

        checkMulticallStatuses(
            multicallStatuses,
            expectedMulticallStatuses,
            true
        );

        assertEq(balanceProtocolPost, balanceProtocolPre);
    }

    function testLiquidateMismatchedBidFail() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        uint256 bidAmount0 = 150;
        contracts[0] = address(searcherA);
        bidInfos[0] = makeBidInfo(bidAmount0, searcherAOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoSearcherContracts(vaultNumber, contracts, bidInfos);

        // mismatched bid--multicall expects higher bid than what is paid out by the searcher
        bidInfos[0].bid = bidInfos[0].bid + 1;

        MulticallData[] memory multicallData = getMulticallData(
            contracts,
            data,
            bidInfos
        );

        uint256 balanceProtocolPre = address(tokenVault).balance;
        AccountBalance memory balancesAPre = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](1);
        expectedMulticallStatuses[0].externalSuccess = false;
        expectedMulticallStatuses[0].multicallRevertReason = "invalid bid";
        expectMulticallIssuedEmit(
            permission,
            multicallData,
            expectedMulticallStatuses
        );

        vm.prank(relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            multicallData
        );

        uint256 balanceProtocolPost = address(tokenVault).balance;
        AccountBalance memory balancesAPost = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        assertEq(balancesAPost.collateral, balancesAPre.collateral);
        assertEq(balancesAPost.debt, balancesAPre.debt);

        checkMulticallStatuses(
            multicallStatuses,
            expectedMulticallStatuses,
            true
        );

        assertEq(balanceProtocolPost, balanceProtocolPre);
    }

    function testLiquidateOpportunityAdapter() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        uint256 bidAmount0 = 150;
        contracts[0] = address(opportunityAdapter);
        bidInfos[0] = makeBidInfo(bidAmount0, searcherAOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoOpportunityAdapter(vaultNumber, bidInfos);

        MulticallData[] memory multicallData = getMulticallData(
            contracts,
            data,
            bidInfos
        );

        AccountBalance memory balancesAPre = getBalances(
            searcherAOwnerAddress,
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );
        uint256 balanceProtocolPre = address(tokenVault).balance;

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](1);
        expectedMulticallStatuses[0].externalSuccess = true;
        expectMulticallIssuedEmit(
            permission,
            multicallData,
            expectedMulticallStatuses
        );

        vm.prank(relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            multicallData
        );

        uint256 balanceProtocolPost = address(tokenVault).balance;

        AccountBalance memory balancesAPost = getBalances(
            searcherAOwnerAddress,
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        assertEq(
            balancesAPost.collateral,
            balancesAPre.collateral + amountsCollateral[vaultNumber]
        );
        assertEq(
            balancesAPost.debt,
            balancesAPre.debt - amountsDebt[vaultNumber]
        );

        checkMulticallStatuses(
            multicallStatuses,
            expectedMulticallStatuses,
            true
        );

        uint256 feeSplitProtocol = expressRelay.getFeeProtocol(
            address(tokenVault)
        );
        uint256 feeSplitPrecision = expressRelay.getFeeSplitPrecision();
        assertEq(
            balanceProtocolPost,
            balanceProtocolPre +
                (bidAmount0 * feeSplitProtocol) /
                feeSplitPrecision
        );
    }

    function testLiquidateOpportunityAdapterInvalidSignatureFail() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        uint256 bidAmount0 = 150;
        contracts[0] = address(opportunityAdapter);
        bidInfos[0] = makeBidInfo(bidAmount0, searcherBOwnerSk);
        bidInfos[0].executor = searcherAOwnerAddress; // use wrong liquidator address to induce invalid signature

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoOpportunityAdapter(vaultNumber, bidInfos);

        MulticallData[] memory multicallData = getMulticallData(
            contracts,
            data,
            bidInfos
        );

        AccountBalance memory balancesAPre = getBalances(
            searcherAOwnerAddress,
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );
        uint256 balanceProtocolPre = address(tokenVault).balance;

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](1);
        expectedMulticallStatuses[0].externalSuccess = false;
        expectedMulticallStatuses[0].externalResult = abi.encodeWithSelector(
            bytes4(keccak256("InvalidSigner()"))
        );
        expectMulticallIssuedEmit(
            permission,
            multicallData,
            expectedMulticallStatuses
        );

        vm.prank(relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            multicallData
        );

        AccountBalance memory balancesAPost = getBalances(
            searcherAOwnerAddress,
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );
        uint256 balanceProtocolPost = address(tokenVault).balance;

        assertEqBalances(balancesAPost, balancesAPre);

        checkMulticallStatuses(
            multicallStatuses,
            expectedMulticallStatuses,
            true
        );

        assertEq(balanceProtocolPost, balanceProtocolPre);
    }

    function testLiquidateOpportunityAdapterExpiredSignatureFail() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        uint256 bidAmount0 = 150;
        contracts[0] = address(opportunityAdapter);
        bidInfos[0] = makeBidInfo(bidAmount0, searcherAOwnerSk);
        bidInfos[0].validUntil = block.timestamp - 1; // use old timestamp for the validUntil field to create expired signature

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoOpportunityAdapter(vaultNumber, bidInfos);

        MulticallData[] memory multicallData = getMulticallData(
            contracts,
            data,
            bidInfos
        );

        AccountBalance memory balancesAPre = getBalances(
            searcherAOwnerAddress,
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );
        uint256 balanceProtocolPre = address(tokenVault).balance;

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](1);
        expectedMulticallStatuses[0].externalSuccess = false;
        expectedMulticallStatuses[0].externalResult = abi.encodeWithSelector(
            bytes4(keccak256("SignatureExpired(uint256)")),
            bidInfos[0].validUntil
        );
        expectMulticallIssuedEmit(
            permission,
            multicallData,
            expectedMulticallStatuses
        );

        vm.prank(relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            multicallData
        );

        AccountBalance memory balancesAPost = getBalances(
            searcherAOwnerAddress,
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );
        uint256 balanceProtocolPost = address(tokenVault).balance;

        assertEqBalances(balancesAPost, balancesAPre);
        assertEq(balanceProtocolPre, balanceProtocolPost);
        checkMulticallStatuses(
            multicallStatuses,
            expectedMulticallStatuses,
            true
        );

        assertEq(balanceProtocolPost, balanceProtocolPre);
    }

    /**
     * @notice Test a multicall with two calls to liquidate the same vault, where the second is expected to fail
     *
     * The second call should fail with the expected error message, bc the vault is already liquidated.
     */
    function testLiquidateLiquidationAdapterLiquidationCallFail() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](2);
        BidInfo[] memory bidInfos = new BidInfo[](2);

        uint256 bidAmount0 = 150;
        contracts[0] = address(opportunityAdapter);
        bidInfos[0] = makeBidInfo(bidAmount0, searcherAOwnerSk);

        uint256 bidAmount1 = 100;
        contracts[1] = address(opportunityAdapter);
        bidInfos[1] = makeBidInfo(bidAmount1, searcherBOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoOpportunityAdapter(vaultNumber, bidInfos);

        MulticallData[] memory multicallData = getMulticallData(
            contracts,
            data,
            bidInfos
        );

        uint256 balanceProtocolPre = address(tokenVault).balance;
        AccountBalance memory balancesAPre = getBalances(
            searcherAOwnerAddress,
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );
        AccountBalance memory balancesBPre = getBalances(
            searcherBOwnerAddress,
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        MulticallStatus[]
            memory expectedMulticallStatuses = new MulticallStatus[](2);
        expectedMulticallStatuses[0].externalSuccess = true;
        expectedMulticallStatuses[1].externalResult = abi.encodeWithSelector(
            TargetCallFailed.selector
        );
        expectMulticallIssuedEmit(
            permission,
            multicallData,
            expectedMulticallStatuses
        );

        vm.prank(relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            multicallData
        );

        uint256 balanceProtocolPost = address(tokenVault).balance;
        AccountBalance memory balancesAPost = getBalances(
            searcherAOwnerAddress,
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );
        AccountBalance memory balancesBPost = getBalances(
            searcherBOwnerAddress,
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        assertEq(
            balancesAPost.collateral,
            balancesAPre.collateral + amountsCollateral[vaultNumber]
        );
        assertEq(
            balancesAPost.debt,
            balancesAPre.debt - amountsDebt[vaultNumber]
        );
        assertEqBalances(balancesBPost, balancesBPre);

        checkMulticallStatuses(
            multicallStatuses,
            expectedMulticallStatuses,
            true
        );

        uint256 feeSplitProtocol = expressRelay.getFeeProtocol(
            address(tokenVault)
        );
        uint256 feeSplitPrecision = expressRelay.getFeeSplitPrecision();
        assertEq(
            balanceProtocolPost,
            balanceProtocolPre +
                (bidAmount0 * feeSplitProtocol) /
                feeSplitPrecision
        );
    }
}

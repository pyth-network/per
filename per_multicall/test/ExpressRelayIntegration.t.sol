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
     *
     * This function creates the entire environment for the start of each test. It is called before each test.
     * This function creates the ExpressRelay, WETH9, OpportunityAdapter, MockPyth, TokenVault, SearcherVault, and two ERC-20 token contracts. The two ERC-20 tokens are used as collateral and debt tokens for the vaults that will be created.
     * It also sets up the initial token amounts for the depositor, searcher A, searcher B, and the token vault. Additionally, it sets the initial oracle prices for the tokens.
     * The function then sets up two vaults in the TokenVault contract. Each vault's collateral and debt tokens are set, as well as the amounts of each token in the vault. Based on the amounts in the vault and the initial token prices, we back out the liquidation threshold prices--these are used later in the tests to set prices that trigger liquidation.
     * Finally, the function funds the searcher wallets with Eth and tokens. It also creates the allowances from the searchers' wallets to the liquidation adapter to use the searcher wallets' tokens and weth to liquidate vaults.
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

        vm.prank(searcherAOwnerAddress, searcherAOwnerAddress);
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

        vm.expectRevert(abi.encodeWithSelector(InvalidLiquidation.selector));
        vm.prank(searcherAOwnerAddress, searcherAOwnerAddress);
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

        contracts[0] = address(searcherA);
        bidInfos[0] = makeBidInfo(150, searcherAOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoSearcherContracts(vaultNumber, bidInfos);

        uint256 balanceProtocolPre = address(tokenVault).balance;
        AccountBalance memory balancesAPre = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        vm.prank(relayer, relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
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

        assertEq(multicallStatuses[0].externalSuccess, true);

        assertExpectedBidPayment(
            balanceProtocolPre,
            balanceProtocolPost,
            bidInfos,
            multicallStatuses
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

        contracts[0] = address(searcherA);
        bidInfos[0] = makeBidInfo(150, searcherAOwnerSk);

        contracts[1] = address(searcherB);
        bidInfos[1] = makeBidInfo(100, searcherAOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoSearcherContracts(vaultNumber, bidInfos);

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

        vm.prank(relayer, relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
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

        // only the first bid should be paid
        assertExpectedBidPayment(
            balanceProtocolPre,
            balanceProtocolPost,
            bidInfos,
            multicallStatuses
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

        contracts[0] = address(searcherA);
        bidInfos[0] = makeBidInfo(150, searcherAOwnerSk);
        contracts[1] = address(searcherB);
        bidInfos[1] = makeBidInfo(100, searcherBOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoSearcherContracts(vaultNumber, bidInfos);

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
        vm.prank(searcherAOwnerAddress, searcherAOwnerAddress);
        searcherA.withdrawEth(address(searcherA).balance);

        vm.prank(relayer, relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
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

        // only the second bid should be paid
        assertExpectedBidPayment(
            balanceProtocolPre,
            balanceProtocolPost,
            bidInfos,
            multicallStatuses
        );
    }

    function testLiquidateWrongPermission() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(searcherA);
        bidInfos[0] = makeBidInfo(150, searcherAOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoSearcherContracts(vaultNumber, bidInfos);

        // wrong permisison key
        permission = abi.encode(address(0));

        AccountBalance memory balancesAPre = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        vm.prank(relayer, relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
        );

        AccountBalance memory balancesAPost = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        assertEq(balancesAPost.collateral, balancesAPre.collateral);
        assertEq(balancesAPost.debt, balancesAPre.debt);

        assertFailedExternal(multicallStatuses[0], "InvalidLiquidation()");
    }

    function testLiquidateMismatchedBid() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(searcherA);
        bidInfos[0] = makeBidInfo(150, searcherAOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoSearcherContracts(vaultNumber, bidInfos);

        // mismatched bid--multicall expects higher bid than what is paid out by the searcher
        bidInfos[0].bid = bidInfos[0].bid + 1;

        AccountBalance memory balancesAPre = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        vm.prank(relayer, relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
        );

        AccountBalance memory balancesAPost = getBalances(
            address(searcherA),
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );

        assertEq(balancesAPost.collateral, balancesAPre.collateral);
        assertEq(balancesAPost.debt, balancesAPre.debt);

        assertFailedMulticall(multicallStatuses[0], "invalid bid");
    }

    function testLiquidateOpportunityAdapter() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(opportunityAdapter);
        bidInfos[0] = makeBidInfo(150, searcherAOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoOpportunityAdapter(vaultNumber, bidInfos);

        AccountBalance memory balancesAPre = getBalances(
            searcherAOwnerAddress,
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );
        uint256 balanceProtocolPre = address(tokenVault).balance;

        vm.prank(relayer, relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
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

        assertEq(multicallStatuses[0].externalSuccess, true);

        assertExpectedBidPayment(
            balanceProtocolPre,
            balanceProtocolPost,
            bidInfos,
            multicallStatuses
        );
    }

    function testLiquidateOpportunityAdapterFailInvalidSignature() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(opportunityAdapter);
        bidInfos[0] = makeBidInfo(150, searcherBOwnerSk);
        bidInfos[0].executor = searcherAOwnerAddress; // use wrong liquidator address to induce invalid signature

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoOpportunityAdapter(vaultNumber, bidInfos);

        AccountBalance memory balancesAPre = getBalances(
            searcherAOwnerAddress,
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );
        uint256 balanceProtocolPre = address(tokenVault).balance;

        vm.prank(relayer, relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
        );

        AccountBalance memory balancesAPost = getBalances(
            searcherAOwnerAddress,
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );
        uint256 balanceProtocolPost = address(tokenVault).balance;

        assertEqBalances(balancesAPost, balancesAPre);
        assertEq(balanceProtocolPre, balanceProtocolPost);

        assertFailedExternal(
            multicallStatuses[0],
            "InvalidExecutorSignature()"
        );
    }

    function testLiquidateOpportunityAdapterFailExpiredSignature() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(opportunityAdapter);
        bidInfos[0] = makeBidInfo(150, searcherAOwnerSk);
        bidInfos[0].validUntil = block.timestamp - 1; // use old timestamp for the validUntil field to create expired signature

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoOpportunityAdapter(vaultNumber, bidInfos);

        AccountBalance memory balancesAPre = getBalances(
            searcherAOwnerAddress,
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );
        uint256 balanceProtocolPre = address(tokenVault).balance;

        vm.prank(relayer, relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
        );

        AccountBalance memory balancesAPost = getBalances(
            searcherAOwnerAddress,
            tokensCollateral[vaultNumber],
            tokensDebt[vaultNumber]
        );
        uint256 balanceProtocolPost = address(tokenVault).balance;

        assertEqBalances(balancesAPost, balancesAPre);
        assertEq(balanceProtocolPre, balanceProtocolPost);
        assertFailedExternal(multicallStatuses[0], "ExpiredSignature()");
    }

    /**
     * @notice Test a multicall with two calls to liquidate the same vault, where the second is expected to fail
     *
     * The second call should fail with the expected error message, bc the vault is already liquidated.
     */
    function testLiquidateLiquidationAdapterFailLiquidationCall() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](2);
        BidInfo[] memory bidInfos = new BidInfo[](2);

        contracts[0] = address(opportunityAdapter);
        contracts[1] = address(opportunityAdapter);
        bidInfos[0] = makeBidInfo(150, searcherAOwnerSk);
        bidInfos[1] = makeBidInfo(100, searcherBOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoOpportunityAdapter(vaultNumber, bidInfos);

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

        vm.prank(relayer, relayer);
        MulticallStatus[] memory multicallStatuses = expressRelay.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
        );

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

        assertEq(multicallStatuses[0].externalSuccess, true);
        assertFailedExternal(multicallStatuses[1], "TargetCallFailed(string)");
    }
}

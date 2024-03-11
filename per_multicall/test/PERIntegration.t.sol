// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {Test, console2} from "forge-std/Test.sol";
import "../src/SigVerify.sol";
import "forge-std/console.sol";
import "forge-std/StdMath.sol";

import {TokenVault} from "../src/TokenVault.sol";
import {SearcherVault} from "../src/SearcherVault.sol";
import {ExpressRelay} from "../src/ExpressRelay.sol";
import {WETH9} from "../src/WETH9.sol";
import {OpportunityAdapter} from "../src/OpportunityAdapter.sol";
import {MyToken} from "../src/MyToken.sol";
import "../src/Errors.sol";
import "../src/TokenVaultErrors.sol";
import "../src/Structs.sol";

import "@pythnetwork/pyth-sdk-solidity/MockPyth.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";

import "openzeppelin-contracts/contracts/utils/Strings.sol";

import "./helpers/Signatures.sol";
import "./helpers/PriceHelpers.sol";
import "./helpers/TestParsingHelpers.sol";
import "./helpers/MulticallHelpers.sol";

/**
 * @title ExpressRelayIntegrationTest
 *
 * ExpressRelayIntegrationTest is a contract that tests the integration of the various contracts in the ExpressRelay stack.
 * This includes the ExpressRelay entrypoint contract for all relay interactions, the TokenVault dummy lending protocol contract, individual searcher contracts programmed to perform liquidations, the OpportunityAdapter contract used to facilitate liquidations directly from searcher EOAs, and the relevant token contracts.
 * We test the integration of these contracts by creating vaults in the TokenVault protocol, simulating undercollateralization of these vaults to trigger liquidations, constructing the necessary liquidation data, and then calling liquidation through OpportunityAdapter or the searcher contracts.
 *
 * The focus in these tests is ensuring that liquidation succeeds (or fails as expected) through the ExpressRelay contrct routing to the searcher contracts or the OpportunityAdapter contract.
 */
contract PERIntegrationTest is
    Test,
    TestParsingHelpers,
    Signatures,
    PriceHelpers,
    MulticallHelpers
{
    TokenVault public tokenVault;
    SearcherVault public searcherA;
    SearcherVault public searcherB;
    ExpressRelay public expressRelay;
    WETH9 public weth;
    OpportunityAdapter public liquidationAdapter;
    MockPyth public mockPyth;

    MyToken public token1;
    MyToken public token2;

    bytes32 idToken1;
    bytes32 idToken2;

    int32 constant tokenExpo = 0;

    address perOperatorAddress;
    uint256 perOperatorSk;
    address searcherAOwnerAddress;
    uint256 searcherAOwnerSk;
    address searcherBOwnerAddress;
    uint256 searcherBOwnerSk;
    address tokenVaultDeployer;
    uint256 tokenVaultDeployerSk;

    uint256 constant healthPrecision = 10 ** 16;

    address depositor; // address of the initial depositor into the token vault

    uint256 constant amountToken1DepositorInit = 1_000_000; // amount of token 1 initially owned by the vault depositor
    uint256 constant amountToken2DepositorInit = 1_000_000; // amount of token 2 initially owned by the vault depositor
    uint256 constant amountToken1AInit = 2_000_000; // amount of token 1 initially owned by searcher A contract
    uint256 constant amountToken2AInit = 2_000_000; // amount of token 2 initially owned by searcher A contract
    uint256 constant amountToken1BInit = 3_000_000; // amount of token 1 initially owned by searcher B contract
    uint256 constant amountToken2BInit = 3_000_000; // amount of token 2 initially owned by searcher B contract
    uint256 constant amountToken2TokenVaultInit = 500_000; // amount of token 2 initially owned by the token vault contract (necessary to allow depositor to borrow token 2)

    address[] tokensCollateral; // addresses of collateral, index corresponds to vault number
    address[] tokensDebt; // addresses of debt, index corresponds to vault number
    uint256[] amountsCollateral; // amounts of collateral, index corresponds to vault number
    uint256[] amountsDebt; // amounts of debt, index corresponds to vault number
    bytes32[] idsCollateral; // pyth price feed ids of collateral, index corresponds to vault number
    bytes32[] idsDebt; // pyth price feed ids of debt, index corresponds to vault number

    // initial token oracle info
    int64 constant token1PriceInitial = 100;
    uint64 constant token1ConfInitial = 1;
    int64 constant token2PriceInitial = 100;
    uint64 constant token2ConfInitial = 1;
    uint64 constant publishTimeInitial = 1_000_000;
    uint64 constant prevPublishTimeInitial = 0;

    int64[] tokenDebtPricesLiqExpressRelay;
    int64[] tokenDebtPricesLiqPermissionless;

    uint256 constant defaultFeeSplitProtocol = 50 * 10 ** 16;

    uint256 feeSplitTokenVault;
    uint256 constant feeSplitPrecisionTokenVault = 10 ** 18;

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

    /**
     * @notice setUpWallets function - sets up the wallets for the test
     *
     * Sets up express relay operator, searcher, initial token vault deployer, and initial vault depositor wallets
     */
    function setUpWallets() public {
        (perOperatorAddress, perOperatorSk) = makeAddrAndKey("perOperator");

        (searcherAOwnerAddress, searcherAOwnerSk) = makeAddrAndKey("searcherA");
        (searcherBOwnerAddress, searcherBOwnerSk) = makeAddrAndKey("searcherB");

        (tokenVaultDeployer, tokenVaultDeployerSk) = makeAddrAndKey(
            "tokenVaultDeployer"
        );

        (depositor, ) = makeAddrAndKey("depositor");
    }

    /**
     * @notice setUpContracts function - sets up the contracts for the test
     *
     * Sets up the ExpressRelay, WETH9, OpportunityAdapter, MockPyth, TokenVault, SearcherVault, and ERC-20 token contracts
     */
    function setUpContracts() public {
        // instantiate multicall contract with ExpressRelay operator as the deployer
        vm.prank(perOperatorAddress, perOperatorAddress);
        expressRelay = new ExpressRelay(
            perOperatorAddress,
            defaultFeeSplitProtocol
        );

        vm.prank(perOperatorAddress, perOperatorAddress);
        weth = new WETH9();

        vm.prank(perOperatorAddress, perOperatorAddress);
        liquidationAdapter = new OpportunityAdapter(
            address(expressRelay),
            address(weth)
        );

        vm.prank(perOperatorAddress, perOperatorAddress);
        mockPyth = new MockPyth(1_000_000, 0);

        vm.prank(tokenVaultDeployer, tokenVaultDeployer); // we prank here to standardize the value of the token contract address across different runs
        tokenVault = new TokenVault(address(expressRelay), address(mockPyth));
        console.log("contract of token vault is", address(tokenVault));
        feeSplitTokenVault = defaultFeeSplitProtocol;

        // instantiate searcher A's contract with searcher A's wallet as the deployer
        vm.prank(searcherAOwnerAddress, searcherAOwnerAddress);
        searcherA = new SearcherVault(
            address(expressRelay),
            address(tokenVault)
        );
        console.log("contract of searcher A is", address(searcherA));

        // instantiate searcher B's contract with searcher B's wallet as the deployer
        vm.prank(searcherBOwnerAddress, searcherBOwnerAddress);
        searcherB = new SearcherVault(
            address(expressRelay),
            address(tokenVault)
        );
        console.log("contract of searcher B is", address(searcherB));

        vm.prank(perOperatorAddress, perOperatorAddress);
        token1 = new MyToken("token1", "T1");
        vm.prank(perOperatorAddress, perOperatorAddress);
        token2 = new MyToken("token2", "T2");
        console.log("contract of token1 is", address(token1));
        console.log("contract of token2 is", address(token2));
    }

    /**
     * @notice setUpTokensAndOracle function - sets up the tokens for the test and their initial oracle feeds
     *
     * Sets up the initial token amounts for the depositor, searcher A, searcher B, and the token vault
     * Also sets the initial oracle prices for the tokens
     */
    function setUpTokensAndOracle() public {
        // mint tokens to the depositor address
        token1.mint(depositor, amountToken1DepositorInit);
        token2.mint(depositor, amountToken2DepositorInit);

        // mint tokens to searcher A contract
        token1.mint(address(searcherA), amountToken1AInit);
        token2.mint(address(searcherA), amountToken2AInit);

        // mint tokens to searcher B contract
        token1.mint(address(searcherB), amountToken1BInit);
        token2.mint(address(searcherB), amountToken2BInit);

        // mint token 2 to the vault contract (to allow creation of initial vault with outstanding debt position)
        token2.mint(address(tokenVault), amountToken2TokenVaultInit);

        // create token price feed IDs
        idToken1 = bytes32(uint256(uint160(address(token1))));
        idToken2 = bytes32(uint256(uint160(address(token2))));

        vm.warp(publishTimeInitial);
        bytes[] memory updateData = new bytes[](2);
        updateData[0] = mockPyth.createPriceFeedUpdateData(
            idToken1,
            token1PriceInitial,
            token1ConfInitial,
            tokenExpo,
            token1PriceInitial,
            token1ConfInitial,
            publishTimeInitial,
            prevPublishTimeInitial
        );
        updateData[1] = mockPyth.createPriceFeedUpdateData(
            idToken2,
            token2PriceInitial,
            token2ConfInitial,
            tokenExpo,
            token2PriceInitial,
            token2ConfInitial,
            publishTimeInitial,
            prevPublishTimeInitial
        );

        mockPyth.updatePriceFeeds(updateData);
    }

    /**
     * @notice setUpVaults function - sets up the vaults for the test and stores relevant info per vault
     */
    function setUpVaults() public {
        // set which tokens are collateral and which are debt for each vault
        tokensCollateral = new address[](2);
        idsCollateral = new bytes32[](2);
        tokensCollateral[0] = address(token1);
        idsCollateral[0] = idToken1;
        tokensCollateral[1] = address(token1);
        idsCollateral[1] = idToken1;

        tokensDebt = new address[](2);
        idsDebt = new bytes32[](2);
        tokensDebt[0] = address(token2);
        idsDebt[0] = idToken2;
        tokensDebt[1] = address(token2);
        idsDebt[1] = idToken2;

        amountsCollateral = new uint256[](2);
        amountsCollateral[0] = 100;
        amountsCollateral[1] = 200;

        amountsDebt = new uint256[](2);
        amountsDebt[0] = 80;
        amountsDebt[1] = 150;

        // create vault 0
        uint256 minCollatPERVault0 = 110 * healthPrecision;
        uint256 minCollatPermissionlessVault0 = 100 * healthPrecision;
        vm.prank(depositor, depositor);
        MyToken(tokensCollateral[0]).approve(
            address(tokenVault),
            amountsCollateral[0]
        );
        vm.prank(depositor, depositor);
        tokenVault.createVault(
            tokensCollateral[0],
            tokensDebt[0],
            amountsCollateral[0],
            amountsDebt[0],
            minCollatPERVault0,
            minCollatPermissionlessVault0,
            idsCollateral[0],
            idsDebt[0],
            new bytes[](0)
        );

        // create vault 1
        uint256 minCollatPERVault1 = 110 * healthPrecision;
        uint256 minCollatPermissionlessVault1 = 100 * healthPrecision;
        vm.prank(depositor, depositor);
        MyToken(tokensCollateral[1]).approve(
            address(tokenVault),
            amountsCollateral[1]
        );
        vm.prank(depositor, depositor);
        tokenVault.createVault(
            tokensCollateral[1],
            tokensDebt[1],
            amountsCollateral[1],
            amountsDebt[1],
            minCollatPERVault1,
            minCollatPermissionlessVault1,
            idsCollateral[1],
            idsDebt[1],
            new bytes[](0)
        );

        int64 priceCollateralVault0;
        int64 priceCollateralVault1;

        if (tokensCollateral[0] == address(token1)) {
            priceCollateralVault0 = token1PriceInitial;
        } else {
            priceCollateralVault0 = token2PriceInitial;
        }

        int64 tokenDebtPriceLiqPermissionlessVault0;
        int64 tokenDebtPriceLiqPERVault0;
        int64 tokenDebtPriceLiqPermissionlessVault1;
        int64 tokenDebtPriceLiqPERVault1;

        tokenDebtPriceLiqPermissionlessVault0 = getDebtLiquidationPrice(
            amountsCollateral[0],
            amountsDebt[0],
            minCollatPermissionlessVault0,
            healthPrecision,
            priceCollateralVault0
        );

        tokenDebtPriceLiqPERVault0 = getDebtLiquidationPrice(
            amountsCollateral[0],
            amountsDebt[0],
            minCollatPERVault0,
            healthPrecision,
            priceCollateralVault0
        );

        if (tokensCollateral[1] == address(token1)) {
            priceCollateralVault1 = token1PriceInitial;
        } else {
            priceCollateralVault1 = token2PriceInitial;
        }

        tokenDebtPriceLiqPermissionlessVault1 = getDebtLiquidationPrice(
            amountsCollateral[1],
            amountsDebt[1],
            minCollatPermissionlessVault1,
            healthPrecision,
            priceCollateralVault1
        );

        tokenDebtPriceLiqPERVault1 = getDebtLiquidationPrice(
            amountsCollateral[1],
            amountsDebt[1],
            minCollatPERVault1,
            healthPrecision,
            priceCollateralVault1
        );

        tokenDebtPricesLiqExpressRelay = new int64[](2);
        tokenDebtPricesLiqExpressRelay[0] = tokenDebtPriceLiqPERVault0;
        tokenDebtPricesLiqExpressRelay[1] = tokenDebtPriceLiqPERVault1;

        tokenDebtPricesLiqPermissionless = new int64[](2);
        tokenDebtPricesLiqPermissionless[
            0
        ] = tokenDebtPriceLiqPermissionlessVault0;
        tokenDebtPricesLiqPermissionless[
            1
        ] = tokenDebtPriceLiqPermissionlessVault1;
    }

    /**
     * @notice fundSearcherWallets function - funds the searcher wallets with Eth, tokens, and allowances
     *
     * Funding enables searchers' wallets to directly liquidate via the liquidation adapter
     */
    function fundSearcherWallets() public {
        // fund searcher A and searcher B
        vm.deal(address(searcherA), 1 ether);
        vm.deal(address(searcherB), 1 ether);

        address[] memory searchers = new address[](2);
        searchers[0] = address(searcherAOwnerAddress);
        searchers[1] = address(searcherBOwnerAddress);

        for (uint256 i = 0; i < searchers.length; i++) {
            address searcher = searchers[i];

            // mint tokens to searcher wallet so it can liquidate vaults
            MyToken(tokensDebt[0]).mint(address(searcher), amountsDebt[0]);
            MyToken(tokensDebt[1]).mint(address(searcher), amountsDebt[1]);

            vm.startPrank(searcher, searcher);

            // create allowance for liquidation adapter
            if (tokensDebt[0] == tokensDebt[1]) {
                MyToken(tokensDebt[0]).approve(
                    address(liquidationAdapter),
                    amountsDebt[0] + amountsDebt[1]
                );
            } else {
                MyToken(tokensDebt[0]).approve(
                    address(liquidationAdapter),
                    amountsDebt[0]
                );
                MyToken(tokensDebt[1]).approve(
                    address(liquidationAdapter),
                    amountsDebt[1]
                );
            }

            // deposit eth into the weth contract
            vm.deal(searcher, (i + 1) * 100 ether);
            weth.deposit{value: (i + 1) * 100 ether}();

            // create allowance for liquidation adapter (weth)
            weth.approve(address(liquidationAdapter), (i + 1) * 100 ether);

            vm.stopPrank();
        }

        // fast forward to enable price updates in the below tests
        vm.warp(publishTimeInitial + 100);
    }

    /**
     * @notice getMulticallInfoSearcherContracts function - creates necessary permission and data for multicall to searcher contracts
     */
    function getMulticallInfoSearcherContracts(
        uint256 vaultNumber,
        BidInfo[] memory bidInfos
    ) public returns (bytes memory permission, bytes[] memory data) {
        vm.roll(2);

        // get permission key
        permission = abi.encode(
            address(tokenVault),
            abi.encodePacked(vaultNumber)
        );

        // raise price of debt token to make vault undercollateralized
        bytes memory tokenDebtUpdateData = createPriceFeedUpdateSimple(
            mockPyth,
            idsDebt[vaultNumber],
            tokenDebtPricesLiqExpressRelay[vaultNumber],
            tokenExpo
        );

        data = new bytes[](bidInfos.length);

        for (uint i = 0; i < bidInfos.length; i++) {
            // create searcher signature
            bytes memory signatureSearcher = createSearcherSignature(
                vaultNumber,
                bidInfos[i].bid,
                bidInfos[i].validUntil,
                bidInfos[i].liquidatorSk
            );
            data[i] = abi.encodeWithSelector(
                searcherA.doLiquidate.selector,
                vaultNumber,
                bidInfos[i].bid,
                bidInfos[i].validUntil,
                tokenDebtUpdateData,
                signatureSearcher
            );
        }
    }

    /**
     * @notice getMulticallInfoOpportunityAdapter function - creates necessary permission and data for multicall to liquidation adapter contract
     */
    function getMulticallInfoOpportunityAdapter(
        uint256 vaultNumber,
        BidInfo[] memory bidInfos
    ) public returns (bytes memory permission, bytes[] memory data) {
        vm.roll(2);

        // get permission key
        permission = abi.encode(
            address(tokenVault),
            abi.encodePacked(vaultNumber)
        );

        // raise price of debt token to make vault undercollateralized
        bytes[] memory updateDatas = new bytes[](1);
        updateDatas[0] = createPriceFeedUpdateSimple(
            mockPyth,
            idsDebt[vaultNumber],
            tokenDebtPricesLiqExpressRelay[vaultNumber],
            tokenExpo
        );

        TokenAmount[] memory repayTokens = new TokenAmount[](1);
        repayTokens[0] = TokenAmount(
            tokensDebt[vaultNumber],
            amountsDebt[vaultNumber]
        );
        TokenAmount[] memory expectedReceiptTokens = new TokenAmount[](1);
        expectedReceiptTokens[0] = TokenAmount(
            tokensCollateral[vaultNumber],
            amountsCollateral[vaultNumber]
        );

        bytes memory calldataVault = abi.encodeWithSelector(
            tokenVault.liquidateWithPriceUpdate.selector,
            vaultNumber,
            updateDatas
        );

        uint256 value = 0;
        address contractAddress = address(tokenVault);

        data = new bytes[](bidInfos.length);

        for (uint i = 0; i < bidInfos.length; i++) {
            // create liquidation call params struct
            bytes memory signatureLiquidator = createLiquidationSignature(
                repayTokens,
                expectedReceiptTokens,
                contractAddress,
                calldataVault,
                value,
                bidInfos[i].bid,
                bidInfos[i].validUntil,
                bidInfos[i].liquidatorSk
            );
            LiquidationCallParams
                memory liquidationCallParams = LiquidationCallParams(
                    repayTokens,
                    expectedReceiptTokens,
                    bidInfos[i].liquidator,
                    contractAddress,
                    calldataVault,
                    value,
                    bidInfos[i].validUntil,
                    bidInfos[i].bid,
                    signatureLiquidator
                );

            data[i] = abi.encodeWithSelector(
                liquidationAdapter.callLiquidation.selector,
                liquidationCallParams
            );
        }
    }

    /**
     * @notice assertExpectedBidPayment function - checks that the expected bid payment is equal to the actual bid payment
     */
    function assertExpectedBidPayment(
        uint256 balancePre,
        uint256 balancePost,
        BidInfo[] memory bidInfos,
        MulticallStatus[] memory multicallStatuses
    ) public {
        require(
            bidInfos.length == multicallStatuses.length,
            "bidInfos and multicallStatuses must have the same length"
        );

        uint256 totalBid = 0;
        string memory emptyRevertReasonString = "";

        for (uint i = 0; i < bidInfos.length; i++) {
            bool externalSuccess = multicallStatuses[i].externalSuccess;
            bool emptyRevertReason = compareStrings(
                multicallStatuses[i].multicallRevertReason,
                emptyRevertReasonString
            );

            if (externalSuccess && emptyRevertReason) {
                totalBid +=
                    (bidInfos[i].bid * feeSplitTokenVault) /
                    feeSplitPrecisionTokenVault;
            }
        }

        assertEq(balancePost, balancePre + totalBid);
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

        uint256 validUntil = 1_000_000_000_000;

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

        uint256 validUntil = 1_000_000_000_000;

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
        bidInfos[0] = makeBidInfo(15, searcherAOwnerSk);

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

        vm.prank(perOperatorAddress, perOperatorAddress);
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
        bidInfos[0] = makeBidInfo(15, searcherAOwnerSk);

        contracts[1] = address(searcherB);
        bidInfos[1] = makeBidInfo(10, searcherAOwnerSk);

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

        vm.prank(perOperatorAddress, perOperatorAddress);
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
        bidInfos[0] = makeBidInfo(15, searcherAOwnerSk);
        contracts[1] = address(searcherB);
        bidInfos[1] = makeBidInfo(10, searcherBOwnerSk);

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

        vm.prank(perOperatorAddress, perOperatorAddress);
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
        bidInfos[0] = makeBidInfo(15, searcherAOwnerSk);

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

        vm.prank(perOperatorAddress, perOperatorAddress);
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
        bidInfos[0] = makeBidInfo(15, searcherAOwnerSk);

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

        vm.prank(perOperatorAddress, perOperatorAddress);
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

        contracts[0] = address(liquidationAdapter);
        bidInfos[0] = makeBidInfo(15, searcherAOwnerSk);

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

        vm.prank(perOperatorAddress, perOperatorAddress);
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

        contracts[0] = address(liquidationAdapter);
        bidInfos[0] = makeBidInfo(15, searcherBOwnerSk);
        bidInfos[0].liquidator = searcherAOwnerAddress; // use wrong liquidator address to induce invalid signature

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

        vm.prank(perOperatorAddress, perOperatorAddress);
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
            "InvalidSearcherSignature()"
        );
    }

    function testLiquidateOpportunityAdapterFailExpiredSignature() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(liquidationAdapter);
        bidInfos[0] = makeBidInfo(15, searcherAOwnerSk);
        bidInfos[0].validUntil = block.number - 1; // use old block number for the validUntil field to create expired signature

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

        vm.prank(perOperatorAddress, perOperatorAddress);
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

        contracts[0] = address(liquidationAdapter);
        contracts[1] = address(liquidationAdapter);
        bidInfos[0] = makeBidInfo(15, searcherAOwnerSk);
        bidInfos[1] = makeBidInfo(10, searcherBOwnerSk);

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

        vm.prank(perOperatorAddress, perOperatorAddress);
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
        assertFailedExternal(
            multicallStatuses[1],
            "LiquidationCallFailed(string)"
        );
    }
}

// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

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
import "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import "openzeppelin-contracts/contracts/utils/Strings.sol";

import "./helpers/Signatures.sol";
import "./helpers/PriceHelpers.sol";
import "./helpers/TestParsingHelpers.sol";
import "./helpers/MulticallHelpers.sol";
import "./helpers/ExpressRelayHarness.sol";
import "../src/OpportunityAdapterUpgradable.sol";
import "../src/ExpressRelayUpgradable.sol";

import "../src/ExpressRelayEvents.sol";
import "../src/ExpressRelayGovernanceEvents.sol";

/**
 * @title ExpressRelayTestSetUp
 *
 * ExpressRelayTestSetup is a contract that defines set up and helper methods for various test suites.
 *
 * The set up methods involve creating the necessary contracts and wallets, and initializing the tokens and vaults.
 * To create a new suite of tests, the new test contract should inherit from this contract and define its setUp() and test functions.
 * Test contracts can derive their setUp() function from setUp... methods defined in this contract.
 *
 * ExpressRelayTestSetup also defines helper methods that are commonly used in the test suites.
 */
contract ExpressRelayTestSetup is
    TestParsingHelpers,
    Signatures,
    PriceHelpers,
    MulticallHelpers,
    ExpressRelayEvents,
    ExpressRelayGovernanceEvents
{
    TokenVault public tokenVault;
    SearcherVault public searcherA;
    SearcherVault public searcherB;
    ExpressRelayUpgradable public expressRelay;
    WETH9 public weth;
    OpportunityAdapterUpgradable public opportunityAdapter;
    MockPyth public mockPyth;

    ExpressRelayHarness public expressRelayHarness;

    MyToken public token1;
    MyToken public token2;

    bytes32 idToken1;
    bytes32 idToken2;

    int32 constant tokenExpo = 0;

    address relayer;
    address admin;
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

    // since feeSplitPrecision is set to 10 ** 18, this represents ~50% of the fees
    uint256 constant feeSplitProtocolDefault = 50 * 10 ** 16;
    // ~5% (10% of the remaining 50%) of the fees go to the relayer
    uint256 constant feeSplitRelayer = 10 ** 17;

    uint256 feeSplitTokenVault;

    /**
     * @notice setUpWallets function - sets up the wallets for the test
     *
     * Sets up express relay operator, searcher, initial token vault deployer, and initial vault depositor wallets
     */
    function setUpWallets() public {
        (relayer, ) = makeAddrAndKey("relayer");
        admin = makeAddr("admin");

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
        vm.prank(relayer);
        ExpressRelayUpgradable _expressRelay = new ExpressRelayUpgradable();
        // deploy proxy contract and point it to implementation
        ERC1967Proxy proxyExpressRelay = new ERC1967Proxy(
            address(_expressRelay),
            ""
        );
        expressRelay = ExpressRelayUpgradable(payable(proxyExpressRelay));
        expressRelay.initialize(
            // TODO: fix the owner and admin here
            relayer,
            admin,
            relayer,
            feeSplitProtocolDefault,
            feeSplitRelayer
        );

        vm.prank(relayer);
        weth = new WETH9();

        vm.prank(relayer);
        OpportunityAdapterUpgradable _opportunityAdapter = new OpportunityAdapterUpgradable();
        // deploy proxy contract and point it to implementation
        ERC1967Proxy proxyOpportunityAdapter = new ERC1967Proxy(
            address(_opportunityAdapter),
            ""
        );
        opportunityAdapter = OpportunityAdapterUpgradable(
            payable(proxyOpportunityAdapter)
        );
        opportunityAdapter.initialize(
            // TODO: fix the owner and admin here
            relayer,
            relayer,
            address(expressRelay),
            address(weth)
        );

        vm.prank(relayer);
        mockPyth = new MockPyth(1_000_000, 0);

        bool allowUndercollateralized = false;
        vm.prank(tokenVaultDeployer); // we prank here to standardize the value of the token contract address across different runs
        tokenVault = new TokenVault(
            admin,
            address(expressRelay),
            address(mockPyth),
            allowUndercollateralized
        );
        console.log("contract of token vault is", address(tokenVault));
        feeSplitTokenVault = feeSplitProtocolDefault;

        // instantiate searcher A's contract with searcher A's wallet as the deployer
        vm.prank(searcherAOwnerAddress);
        searcherA = new SearcherVault(
            address(expressRelay),
            address(tokenVault)
        );
        console.log("contract of searcher A is", address(searcherA));

        // instantiate searcher B's contract with searcher B's wallet as the deployer
        vm.prank(searcherBOwnerAddress);
        searcherB = new SearcherVault(
            address(expressRelay),
            address(tokenVault)
        );
        console.log("contract of searcher B is", address(searcherB));

        vm.prank(relayer);
        token1 = new MyToken("token1", "T1");
        vm.prank(relayer);
        token2 = new MyToken("token2", "T2");
        console.log("contract of token1 is", address(token1));
        console.log("contract of token2 is", address(token2));
    }

    /**
     * @notice setUpExpressRelayHarness function - sets up the ExpressRelayHarness contract for internal function tests
     */
    function setUpExpressRelayHarness() public {
        vm.prank(relayer);
        ExpressRelayHarness _expressRelay = new ExpressRelayHarness();
        ERC1967Proxy proxyExpressRelay = new ERC1967Proxy(
            address(_expressRelay),
            ""
        );
        expressRelayHarness = ExpressRelayHarness(payable(proxyExpressRelay));
        expressRelayHarness.initialize(
            // TODO: fix the owner and admin here
            relayer,
            admin,
            relayer,
            feeSplitProtocolDefault,
            feeSplitRelayer
        );
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
        vm.prank(depositor);
        MyToken(tokensCollateral[0]).approve(
            address(tokenVault),
            amountsCollateral[0]
        );
        vm.prank(depositor);
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
        vm.prank(depositor);
        MyToken(tokensCollateral[1]).approve(
            address(tokenVault),
            amountsCollateral[1]
        );
        vm.prank(depositor);
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

            // create allowance for opportunity adapter
            if (tokensDebt[0] == tokensDebt[1]) {
                MyToken(tokensDebt[0]).approve(
                    address(opportunityAdapter),
                    amountsDebt[0] + amountsDebt[1]
                );
            } else {
                MyToken(tokensDebt[0]).approve(
                    address(opportunityAdapter),
                    amountsDebt[0]
                );
                MyToken(tokensDebt[1]).approve(
                    address(opportunityAdapter),
                    amountsDebt[1]
                );
            }

            // deposit eth into the weth contract
            vm.deal(searcher, (i + 1) * 100 ether);
            weth.deposit{value: (i + 1) * 100 ether}();

            // create allowance for opportunity adapter (weth)
            weth.approve(address(opportunityAdapter), (i + 1) * 100 ether);

            vm.stopPrank();
        }

        // fast forward to enable price updates in the below tests
        vm.warp(publishTimeInitial + 100);
    }

    /**
     * @notice getMulticallInfoSearcherContracts function - creates necessary permission and data for multicall to searcher contracts
     *
     * @param vaultNumber: the vault number to liquidate
     * @param bidInfos: array of BidInfo structs containing bid amount, validUntil, executor address, and executor secret key
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
                bidInfos[i].executorSk
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
     *
     * @param vaultNumber: the vault number to liquidate
     * @param bidInfos: array of BidInfo structs containing bid amount, validUntil, executor address, and executor secret key
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

        TokenAmount[] memory sellTokens = new TokenAmount[](1);
        sellTokens[0] = TokenAmount(
            tokensDebt[vaultNumber],
            amountsDebt[vaultNumber]
        );
        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        buyTokens[0] = TokenAmount(
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
            ExecutionParams
                memory executionParams = createAndSignExecutionParams(
                    sellTokens,
                    buyTokens,
                    contractAddress,
                    calldataVault,
                    value,
                    bidInfos[i].bid,
                    bidInfos[i].validUntil,
                    bidInfos[i].executorSk
                );

            // manually set the executor address again since it's not necessarily the same as vm.addr(executorSk)
            executionParams.executor = bidInfos[i].executor;

            data[i] = abi.encodeWithSelector(
                opportunityAdapter.executeOpportunity.selector,
                executionParams
            );
        }
    }

    /**
     * @notice getRandomBidId function - generates a random bid ID based on the target contract, target calldata, and bid amount
     *
     * @param targetContract: the target contract address
     * @param targetCalldata: the target calldata
     * @param bidAmount: the bid amount
     */
    function getRandomBidId(
        address targetContract,
        bytes memory targetCalldata,
        uint256 bidAmount
    ) public pure returns (bytes16) {
        return
            bytes16(
                keccak256(
                    abi.encodePacked(
                        keccak256("randomBidIdConstruction"),
                        targetContract,
                        targetCalldata,
                        bidAmount
                    )
                )
            );
    }

    /**
     * @notice getMulticallData function - creates necessary data for multicall to multiple contracts
     *
     * @param contracts: array of contract addresses
     * @param data: array of calldata
     * @param bidInfos: array of BidInfo structs containing bid amount, validUntil, executor address, and executor secret key
     */
    function getMulticallData(
        address[] memory contracts,
        bytes[] memory data,
        BidInfo[] memory bidInfos
    ) public pure returns (MulticallData[] memory multicallData) {
        require(
            (contracts.length == data.length) &&
                (data.length == bidInfos.length),
            "contracts, data, and bidAmounts must have the same length"
        );
        uint256[] memory bidAmounts = extractBidAmounts(bidInfos);

        multicallData = new MulticallData[](contracts.length);
        for (uint i = 0; i < contracts.length; i++) {
            bytes16 bidId = getRandomBidId(
                contracts[i],
                data[i],
                bidAmounts[i]
            );
            multicallData[i] = MulticallData(
                bidId,
                contracts[i],
                data[i],
                bidAmounts[i]
            );
        }
    }

    struct BalancesMockTarget {
        uint256 balanceFeeReceiver;
        uint256 balanceMockTarget;
        uint256 balanceExpressRelay;
        uint256 balanceRelayer;
    }

    /**
     * @notice getBalances function - gets the balances of the fee receiver, MockTarget target contract, express relay, and relayer
     *
     * @param feeReceiver: the address of the fee receiver
     * @param mockTarget: the address of the MockTarget contract
     */
    function getBalancesMockTarget(
        address feeReceiver,
        address mockTarget
    ) public view returns (BalancesMockTarget memory) {
        return
            BalancesMockTarget(
                feeReceiver.balance,
                mockTarget.balance,
                address(expressRelay).balance,
                relayer.balance
            );
    }

    /**
     * @notice assertExpectedBidPaymentMockTarget function - checks that the expected bid payments across fee receiver, target contract, express relay, and relayer are equal to the actual payments
     *
     * @param balancesPre: the balances of the fee receiver, target contract, express relay, and relayer before the bid
     * @param balancesPost: the balances of the fee receiver, target contract, express relay, and relayer after the bid
     * @param bidInfos: array of BidInfo structs containing bid amount, validUntil, executor address, and executor secret key
     * @param multicallStatuses: array of MulticallStatus structs containing external success, result, and revert reason
     */
    function assertExpectedBidPaymentMockTarget(
        BalancesMockTarget memory balancesPre,
        BalancesMockTarget memory balancesPost,
        BidInfo[] memory bidInfos,
        MulticallStatus[] memory multicallStatuses
    ) public {
        require(
            bidInfos.length == multicallStatuses.length,
            "bidInfos and multicallStatuses must have the same length"
        );

        BalancesMockTarget memory fees;

        string memory emptyRevertReasonString = "";

        for (uint i = 0; i < bidInfos.length; i++) {
            bool emptyRevertReason = compareStrings(
                multicallStatuses[i].multicallRevertReason,
                emptyRevertReasonString
            );

            if (multicallStatuses[i].externalSuccess && emptyRevertReason) {
                fees.balanceMockTarget += bidInfos[i].bid;

                uint256 protocolSplit = (bidInfos[i].bid *
                    expressRelay.getFeeProtocolDefault()) /
                    expressRelay.getFeeSplitPrecision();
                fees.balanceFeeReceiver += protocolSplit;

                uint256 remainder = (bidInfos[i].bid - protocolSplit);
                uint256 relayerSplit = (remainder * feeSplitRelayer) /
                    expressRelay.getFeeSplitPrecision();
                fees.balanceRelayer += relayerSplit;

                fees.balanceExpressRelay += remainder - relayerSplit;
            }
        }

        assertEq(
            balancesPost.balanceFeeReceiver,
            balancesPre.balanceFeeReceiver + fees.balanceFeeReceiver
        );
        assertEq(
            balancesPost.balanceMockTarget,
            balancesPre.balanceMockTarget - fees.balanceMockTarget
        );
        assertEq(
            balancesPost.balanceExpressRelay,
            balancesPre.balanceExpressRelay + fees.balanceExpressRelay
        );
        assertEq(
            balancesPost.balanceRelayer,
            balancesPre.balanceRelayer + fees.balanceRelayer
        );
    }

    /**
     * @notice runChecksMockTarget function - runs MulticallStatus and payment checks for interactions with MockTarget contract
     *
     * @param feeReceiver: the address of the fee receiver
     * @param mockTargetAddress: the address of the MockTarget contract
     * @param multicallStatuses: array of MulticallStatus structs containing external success, result, and revert reason
     * @param expectedMulticallStatuses: expected values for MulticallStatus structs
     * @param balancesPre: the balances of the fee receiver, target contract, express relay, and relayer before the bid
     * @param bidInfos: array of BidInfo structs containing bid amount, validUntil, executor address, and executor secret key
     */
    function runChecksMockTarget(
        address feeReceiver,
        address mockTargetAddress,
        MulticallStatus[] memory multicallStatuses,
        MulticallStatus[] memory expectedMulticallStatuses,
        BalancesMockTarget memory balancesPre,
        BidInfo[] memory bidInfos
    ) public {
        BalancesMockTarget memory balancesPost = getBalancesMockTarget(
            feeReceiver,
            mockTargetAddress
        );

        checkMulticallStatuses(
            multicallStatuses,
            expectedMulticallStatuses,
            false
        );

        assertExpectedBidPaymentMockTarget(
            balancesPre,
            balancesPost,
            bidInfos,
            multicallStatuses
        );
    }

    /**
     * @notice makeMulticallMockTargetCall function - creates necessary permission, balances, and data for multicall to MockTarget contract
     *
     * @param mockTargetAddress: the address of the MockTarget contract
     * @param feeReceiver: the address of the fee receiver
     * @param contracts: array of target contract addresses
     * @param data: array of target calldata
     * @param bidInfos: array of BidInfo structs containing bid amount, validUntil, executor address, and executor secret key
     */
    function makeMulticallMockTargetCall(
        address mockTargetAddress,
        address feeReceiver,
        address[] memory contracts,
        bytes[] memory data,
        BidInfo[] memory bidInfos
    )
        public
        view
        returns (
            bytes memory permission,
            BalancesMockTarget memory balancesPre,
            MulticallData[] memory multicallData
        )
    {
        permission = abi.encode(address(feeReceiver), abi.encode(uint256(0)));

        balancesPre = getBalancesMockTarget(feeReceiver, mockTargetAddress);

        multicallData = getMulticallData(contracts, data, bidInfos);
    }

    /**
     * @notice expectMulticallIssuedEmit function - emits the expected MulticallIssued event for each loop of multicall for the given data
     *
     * @param permission: the permission key
     * @param multicallData: array of MulticallData structs containing bid ID, bid amount, target contract address, and target calldata
     * @param expectedMulticallStatuses: expected values for MulticallStatus structs
     */
    function expectMulticallIssuedEmit(
        bytes memory permission,
        MulticallData[] memory multicallData,
        MulticallStatus[] memory expectedMulticallStatuses
    ) internal {
        require(
            multicallData.length == expectedMulticallStatuses.length,
            "Multicall data and status length mismatch"
        );
        for (uint i = 0; i < multicallData.length; i++) {
            // TODO: maybe check the data as well, eventually--currently sometimes difficult to pin down exact multicallRevertReason
            vm.expectEmit(true, true, true, false, address(expressRelay));
            emit MulticallIssued(
                permission,
                i,
                multicallData[i].bidId,
                multicallData[i].bidAmount,
                expectedMulticallStatuses[i]
            );
        }
    }

    /**
     * @notice generateRandomPermission function - generates a random permission for testing purposes
     */
    function generateRandomPermission()
        public
        returns (
            address protocol,
            bytes memory permissionId,
            bytes memory permission
        )
    {
        protocol = makeAddr("protocol");
        permissionId = abi.encode("random permission id");
        permission = abi.encode(protocol, permissionId);
    }
}

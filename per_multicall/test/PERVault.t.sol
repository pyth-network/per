// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Test, console2} from "forge-std/Test.sol";
import "../src/SigVerify.sol";
import "forge-std/console.sol";
import "forge-std/StdMath.sol";

import {TokenVault} from "../src/TokenVault.sol";
import {SearcherVault} from "../src/SearcherVault.sol";
import {PERMulticall} from "../src/PERMulticall.sol";
import {WETH9} from "../src/WETH9.sol";
import {LiquidationAdapter} from "../src/LiquidationAdapter.sol";
import {MyToken} from "../src/MyToken.sol";
import "../src/Errors.sol";
import "../src/TokenVaultErrors.sol";
import "../src/Structs.sol";

import "@pythnetwork/pyth-sdk-solidity/MockPyth.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";

import "openzeppelin-contracts/contracts/utils/Strings.sol";

import "../src/Errors.sol";

import "./helpers/ErrorChecks.sol";
import "./helpers/Signatures.sol";
import "./helpers/PriceHelpers.sol";

contract PERVaultTest is Test, Signatures, ErrorChecks, PriceHelpers {
    TokenVault public tokenVault;
    SearcherVault public searcherA;
    SearcherVault public searcherB;
    PERMulticall public multicall;
    WETH9 public weth;
    LiquidationAdapter public liquidationAdapter;
    MockPyth public mockPyth;

    MyToken public token1;
    MyToken public token2;

    bytes32 _idToken1;
    bytes32 _idToken2;

    int32 _tokenExpo = 0;

    address _perOperatorAddress;
    uint256 _perOperatorSk; // address public immutable _perOperatorAddress = address(88);
    address _searcherAOwnerAddress;
    uint256 _searcherAOwnerSk;
    address _searcherBOwnerAddress;
    uint256 _searcherBOwnerSk;
    address _tokenVaultDeployer;
    uint256 _tokenVaultDeployerSk;

    uint256 public healthPrecision = 10 ** 16;

    address _depositor = address(44);

    uint256 _q1Depositor; // quantity of token 1 initially owned by the vault depositor
    uint256 _q2Depositor; // quantity of token 2 initially owned by the vault depositor
    uint256 _q1A; // quantity of token 1 initially owned by searcher A contract
    uint256 _q2A; // quantity of token 2 initially owned by searcher A contract
    uint256 _q1B; // quantity of token 1 initially owned by searcher B contract
    uint256 _q2B; // quantity of token 2 initially owned by searcher B contract
    uint256 _q2TokenVault; // quantity of token 2 initially owned by the token vault contract (necessary to allow depositor to borrow token 2)

    // these are used for assert checks in the tests
    uint256 _qCollateralA; // quantity of collateral initially owned by searcher A
    uint256 _qDebtA; // quantity of debt initially owned by searcher A
    uint256 _qCollateralB; // quantity of collateral initially owned by searcher B
    uint256 _qDebtB; // quantity of debt initially owned by searcher B

    address[] tokensCollateral; // addresses of collateral, index corresponds to vault number
    address[] tokensDebt; // addresses of debt, index corresponds to vault number
    uint256[] amountsCollateral; // amounts of collateral, index corresponds to vault number
    uint256[] amountsDebt; // amounts of debt, index corresponds to vault number
    bytes32[] idsCollateral; // pyth price feed ids of collateral, index corresponds to vault number
    bytes32[] idsDebt; // pyth price feed ids of debt, index corresponds to vault number

    int64 _tokenDebtPriceLiqPermissionlessVault0;
    int64 _tokenDebtPriceLiqPERVault0;
    int64 _tokenDebtPriceLiqPermissionlessVault1;
    int64 _tokenDebtPriceLiqPERVault1;
    int64[] _tokenDebtPricesLiqPER;
    int64[] _tokenDebtPricesLiqPermissionless;

    uint256 _defaultFeeSplitProtocol;

    uint256 _feeSplitTokenVault;
    uint256 _feeSplitPrecisionTokenVault = 10 ** 18;

    uint256 _signaturePerVersionNumber = 0;

    function setUp() public {
        // make PER operator wallet
        (_perOperatorAddress, _perOperatorSk) = makeAddrAndKey("perOperator");
        console.log("pk per operator", _perOperatorSk);

        _defaultFeeSplitProtocol = 50 * 10 ** 16;

        // instantiate multicall contract with PER operator as sender/origin
        vm.prank(_perOperatorAddress, _perOperatorAddress);
        multicall = new PERMulticall(
            _perOperatorAddress,
            _defaultFeeSplitProtocol
        );

        // instantiate weth contract
        vm.prank(_perOperatorAddress, _perOperatorAddress);
        weth = new WETH9();

        // instantiate liquidation adapter contract
        vm.prank(_perOperatorAddress, _perOperatorAddress);
        liquidationAdapter = new LiquidationAdapter(
            address(multicall),
            address(weth)
        );

        // make searcherA and searcherB wallets
        (_searcherAOwnerAddress, _searcherAOwnerSk) = makeAddrAndKey(
            "searcherA"
        );
        (_searcherBOwnerAddress, _searcherBOwnerSk) = makeAddrAndKey(
            "searcherB"
        );
        console.log("pk searcherA", _searcherAOwnerSk);
        console.log("pk searcherB", _searcherBOwnerSk);

        (_tokenVaultDeployer, _tokenVaultDeployerSk) = makeAddrAndKey(
            "tokenVaultDeployer"
        );
        console.log("pk token vault deployer", _tokenVaultDeployerSk);

        // instantiate mock pyth contract
        vm.prank(_perOperatorAddress, _perOperatorAddress);
        mockPyth = new MockPyth(1_000_000, 0);

        // instantiate token vault contract
        vm.prank(_tokenVaultDeployer, _tokenVaultDeployer); // we prank here to standardize the value of the token contract address across different runs
        tokenVault = new TokenVault(address(multicall), address(mockPyth));
        console.log("contract of token vault is", address(tokenVault));
        _feeSplitTokenVault = _defaultFeeSplitProtocol;

        // instantiate searcher A's contract with searcher A as sender/origin
        vm.prank(_searcherAOwnerAddress, _searcherAOwnerAddress);
        searcherA = new SearcherVault(address(multicall), address(tokenVault));
        console.log("contract of searcher A is", address(searcherA));

        // instantiate searcher B's contract with searcher B as sender/origin
        vm.prank(_searcherBOwnerAddress, _searcherBOwnerAddress);
        searcherB = new SearcherVault(address(multicall), address(tokenVault));
        console.log("contract of searcher B is", address(searcherB));

        // instantiate ERC-20 tokens
        vm.prank(_perOperatorAddress, _perOperatorAddress);
        token1 = new MyToken("token1", "T1");
        vm.prank(_perOperatorAddress, _perOperatorAddress);
        token2 = new MyToken("token2", "T2");
        console.log("contract of token1 is", address(token1));
        console.log("contract of token2 is", address(token2));

        _q1Depositor = 1_000_000;
        _q2Depositor = 1_000_000;
        _q1A = 2_000_000;
        _q2A = 2_000_000;
        _q1B = 3_000_000;
        _q2B = 3_000_000;
        _q2TokenVault = 500_000;

        // mint tokens to the _depositor address
        token1.mint(_depositor, _q1Depositor);
        token2.mint(_depositor, _q2Depositor);

        // mint tokens to searcher A contract
        token1.mint(address(searcherA), _q1A);
        token2.mint(address(searcherA), _q2A);

        // mint tokens to searcher B contract
        token1.mint(address(searcherB), _q1B);
        token2.mint(address(searcherB), _q2B);

        // mint token 2 to the vault contract (to allow creation of initial vault with outstanding debt position)
        token2.mint(address(tokenVault), _q2TokenVault);

        // create token price feed IDs
        _idToken1 = bytes32(uint256(uint160(address(token1))));
        _idToken2 = bytes32(uint256(uint160(address(token2))));

        // set initial oracle prices
        int64 token1Price = 100;
        uint64 token1Conf = 1;

        int64 token2Price = 100;
        uint64 token2Conf = 1;

        uint64 publishTime = 1_000_000;
        uint64 prevPublishTime = 0;

        vm.warp(publishTime);
        bytes memory token1UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken1,
            token1Price,
            token1Conf,
            _tokenExpo,
            token1Price,
            token1Conf,
            publishTime,
            prevPublishTime
        );
        bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken2,
            token2Price,
            token2Conf,
            _tokenExpo,
            token2Price,
            token2Conf,
            publishTime,
            prevPublishTime
        );

        bytes[] memory updateData = new bytes[](2);

        updateData[0] = token1UpdateData;
        updateData[1] = token2UpdateData;

        mockPyth.updatePriceFeeds(updateData);

        tokensCollateral = new address[](2);
        idsCollateral = new bytes32[](2);
        tokensCollateral[0] = address(token1);
        idsCollateral[0] = _idToken1;
        tokensCollateral[1] = address(token1);
        idsCollateral[1] = _idToken1;

        tokensDebt = new address[](2);
        idsDebt = new bytes32[](2);
        tokensDebt[0] = address(token2);
        idsDebt[0] = _idToken2;
        tokensDebt[1] = address(token2);
        idsDebt[1] = _idToken2;

        amountsCollateral = new uint256[](2);
        amountsCollateral[0] = 100;
        amountsCollateral[1] = 200;

        amountsDebt = new uint256[](2);
        amountsDebt[0] = 80;
        amountsDebt[1] = 150;

        if (
            (tokensCollateral[0] == address(token1)) &&
            (tokensDebt[0] == address(token2))
        ) {
            _qCollateralA = _q1A;
            _qDebtA = _q2A;
            _qCollateralB = _q1B;
            _qDebtB = _q2B;
        } else if (
            (tokensCollateral[0] == address(token2)) &&
            (tokensDebt[0] == address(token1))
        ) {
            _qCollateralA = _q2A;
            _qDebtA = _q1A;
            _qCollateralB = _q2B;
            _qDebtB = _q1B;
        }

        // create vault 0
        uint256 minCollatPERVault0 = 110 * healthPrecision;
        uint256 minCollatPermissionlessVault0 = 100 * healthPrecision;
        vm.prank(_depositor, _depositor);
        MyToken(tokensCollateral[0]).approve(
            address(tokenVault),
            amountsCollateral[0]
        );
        vm.prank(_depositor, _depositor);
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
        _q1Depositor -= amountsCollateral[0];
        _q2Depositor += amountsDebt[0];

        // create vault 1
        uint256 minCollatPERVault1 = 110 * healthPrecision;
        uint256 minCollatPermissionlessVault1 = 100 * healthPrecision;
        vm.prank(_depositor, _depositor);
        MyToken(tokensCollateral[1]).approve(
            address(tokenVault),
            amountsCollateral[1]
        );
        vm.prank(_depositor, _depositor);
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
        _q1Depositor -= amountsCollateral[0];
        _q2Depositor += amountsDebt[0];

        int64 priceCollateral0;
        int64 priceCollateral1;

        if (tokensCollateral[0] == address(token1)) {
            priceCollateral0 = token1Price;
        } else {
            priceCollateral0 = token2Price;
        }

        _tokenDebtPriceLiqPermissionlessVault0 = getDebtLiquidationPrice(
            amountsCollateral[0],
            amountsDebt[0],
            minCollatPermissionlessVault0,
            healthPrecision,
            priceCollateral0
        );

        _tokenDebtPriceLiqPERVault0 = getDebtLiquidationPrice(
            amountsCollateral[0],
            amountsDebt[0],
            minCollatPERVault0,
            healthPrecision,
            priceCollateral0
        );

        if (tokensCollateral[1] == address(token1)) {
            priceCollateral1 = token1Price;
        } else {
            priceCollateral1 = token2Price;
        }

        _tokenDebtPriceLiqPermissionlessVault1 = getDebtLiquidationPrice(
            amountsCollateral[1],
            amountsDebt[1],
            minCollatPermissionlessVault1,
            healthPrecision,
            priceCollateral1
        );

        _tokenDebtPriceLiqPERVault1 = getDebtLiquidationPrice(
            amountsCollateral[1],
            amountsDebt[1],
            minCollatPERVault1,
            healthPrecision,
            priceCollateral1
        );

        _tokenDebtPricesLiqPER = new int64[](2);
        _tokenDebtPricesLiqPER[0] = _tokenDebtPriceLiqPERVault0;
        _tokenDebtPricesLiqPER[1] = _tokenDebtPriceLiqPERVault1;

        _tokenDebtPricesLiqPermissionless = new int64[](2);
        _tokenDebtPricesLiqPermissionless[
            0
        ] = _tokenDebtPriceLiqPermissionlessVault0;
        _tokenDebtPricesLiqPermissionless[
            1
        ] = _tokenDebtPriceLiqPermissionlessVault1;

        // fund searcher A and searcher B
        vm.deal(address(searcherA), 1 ether);
        vm.deal(address(searcherB), 1 ether);

        address[] memory searchers = new address[](2);
        searchers[0] = address(_searcherAOwnerAddress);
        searchers[1] = address(_searcherBOwnerAddress);

        for (uint256 i = 0; i < searchers.length; i++) {
            address searcher = searchers[i];

            // mint tokens to searcher wallet so it can liquidate vault 0
            MyToken(tokensDebt[0]).mint(address(searcher), amountsDebt[0]);

            vm.startPrank(searcher, searcher);

            // create allowance for liquidation adapter (token 2)
            MyToken(tokensDebt[0]).approve(
                address(liquidationAdapter),
                amountsDebt[0]
            );

            // deposit eth into the weth contract
            vm.deal(searcher, (i + 1) * 100 ether);
            weth.deposit{value: (i + 1) * 100 ether}();

            // create allowance for liquidation adapter (weth)
            weth.approve(address(liquidationAdapter), (i + 1) * 100 ether);

            vm.stopPrank();
        }

        // fast forward to enable price updates in the below tests
        vm.warp(publishTime + 100);
    }

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
            _tokenDebtPricesLiqPER[vaultNumber],
            _tokenExpo
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

    function getMulticallInfoLiquidationAdapter(
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
            _tokenDebtPricesLiqPER[vaultNumber],
            _tokenExpo
        );

        TokenQty[] memory repayTokens = new TokenQty[](1);
        repayTokens[0] = TokenQty(
            tokensDebt[vaultNumber],
            amountsDebt[vaultNumber]
        );
        TokenQty[] memory expectedReceiptTokens = new TokenQty[](1);
        expectedReceiptTokens[0] = TokenQty(
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

            data[i] = abi.encodeWithSignature(
                "callLiquidation(((address,uint256)[],(address,uint256)[],address,address,bytes,uint256,uint256,uint256,bytes))",
                liquidationCallParams
            );
        }
    }

    function testLiquidateNoPER() public {
        uint vaultNumber = 0;
        // test permissionless liquidation (success)
        // raise price of debt token to make vault 0 undercollateralized
        bytes memory tokenDebtUpdateData = createPriceFeedUpdateSimple(
            mockPyth,
            idsDebt[vaultNumber],
            _tokenDebtPriceLiqPermissionlessVault0,
            _tokenExpo
        );

        bytes memory signatureSearcher;

        uint256 validUntil = 1_000_000_000_000;

        vm.prank(_searcherAOwnerAddress, _searcherAOwnerAddress);
        searcherA.doLiquidate(
            0,
            0,
            validUntil,
            tokenDebtUpdateData,
            signatureSearcher
        );

        AccountBalance memory balancesAPost = getBalances(
            address(searcherA),
            vaultNumber
        );

        assertEq(
            balancesAPost.collateral,
            _qCollateralA + amountsCollateral[vaultNumber]
        );
        assertEq(
            MyToken(tokensDebt[vaultNumber]).balanceOf(address(searcherA)),
            _qDebtA - amountsDebt[vaultNumber]
        );
    }

    function testLiquidateNoPERFail() public {
        uint vaultNumber = 0;
        // test permissionless liquidation (failure)
        // raise price of debt token to make vault 0 undercollateralized
        bytes memory tokenDebtUpdateData = createPriceFeedUpdateSimple(
            mockPyth,
            idsDebt[vaultNumber],
            _tokenDebtPriceLiqPERVault0,
            _tokenExpo
        );

        bytes memory signatureSearcher;

        uint256 validUntil = 1_000_000_000_000;

        vm.expectRevert(abi.encodeWithSelector(InvalidLiquidation.selector));
        vm.prank(_searcherAOwnerAddress, _searcherAOwnerAddress);
        searcherA.doLiquidate(
            0,
            0,
            validUntil,
            tokenDebtUpdateData,
            signatureSearcher
        );
    }

    function testLiquidateSingle() public {
        // test PER path liquidation (via multicall, per operator calls) with searcher contract
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(searcherA);
        bidInfos[0] = makeBidInfo(15, _searcherAOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoSearcherContracts(vaultNumber, bidInfos);

        uint256 balanceProtocolPre = address(tokenVault).balance;

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
        );

        uint256 balanceProtocolPost = address(tokenVault).balance;

        assertEq(
            MyToken(tokensCollateral[vaultNumber]).balanceOf(
                address(searcherA)
            ),
            _qCollateralA + amountsCollateral[vaultNumber]
        );
        assertEq(
            MyToken(tokensDebt[vaultNumber]).balanceOf(address(searcherA)),
            _qDebtA - amountsDebt[vaultNumber]
        );

        assertEq(multicallStatuses[0].externalSuccess, true);

        assertEq(
            balanceProtocolPost - balanceProtocolPre,
            (bidInfos[0].bid * _feeSplitTokenVault) /
                _feeSplitPrecisionTokenVault
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
        bidInfos[0] = makeBidInfo(15, _searcherAOwnerSk);

        contracts[1] = address(searcherB);
        bidInfos[1] = makeBidInfo(10, _searcherAOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoSearcherContracts(vaultNumber, bidInfos);

        uint256 balanceProtocolPre = address(tokenVault).balance;

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
        );

        uint256 balanceProtocolPost = address(tokenVault).balance;
        AccountBalance memory balancesAPost = getBalances(
            address(searcherA),
            vaultNumber
        );

        AccountBalance memory balancesBPost = getBalances(
            address(searcherB),
            vaultNumber
        );

        assertEq(
            balancesAPost.collateral,
            _qCollateralA + amountsCollateral[vaultNumber]
        );
        assertEq(balancesAPost.debt, _qDebtA - amountsDebt[vaultNumber]);

        assertEq(balancesBPost.collateral, _qCollateralB);
        assertEq(balancesBPost.debt, _qDebtB);

        logMulticallStatuses(multicallStatuses);

        // only the first bid should be paid
        assertEq(
            balanceProtocolPost - balanceProtocolPre,
            (bidInfos[0].bid * _feeSplitTokenVault) /
                _feeSplitPrecisionTokenVault
        );
    }

    function logMulticallStatuses(
        MulticallStatus[] memory multicallStatuses
    ) internal view {
        for (uint256 i = 0; i < multicallStatuses.length; i++) {
            console.log("External Success:");
            console.log(multicallStatuses[i].externalSuccess);
            console.log("External Result:");
            console.logBytes(multicallStatuses[i].externalResult);
            console.log("Multicall Revert reason:");
            console.log(multicallStatuses[i].multicallRevertReason);
            console.log("----------------------------");
        }
    }

    /**
     * @notice Test a multicall with two calls, where the first is expected to fail
     *
     * The first call should fail, bc the searcher contract has no Eth to pay the PER operator. The second should therefore succeed in liquidating the vault.
     */
    function testLiquidateMultipleFailFirst() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](2);
        BidInfo[] memory bidInfos = new BidInfo[](2);

        contracts[0] = address(searcherA);
        bidInfos[0] = makeBidInfo(15, _searcherAOwnerSk);
        contracts[1] = address(searcherB);
        bidInfos[1] = makeBidInfo(10, _searcherBOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoSearcherContracts(vaultNumber, bidInfos);

        uint256 balanceProtocolPre = address(tokenVault).balance;

        // drain searcherA contract of Eth, so that the first liquidation fails
        vm.prank(_searcherAOwnerAddress, _searcherAOwnerAddress);
        searcherA.withdrawEth(address(searcherA).balance);

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
        );

        uint256 balanceProtocolPost = address(tokenVault).balance;

        AccountBalance memory balancesAPost = getBalances(
            address(searcherA),
            vaultNumber
        );
        AccountBalance memory balancesBPost = getBalances(
            address(searcherB),
            vaultNumber
        );

        assertEq(balancesAPost.collateral, _qCollateralA);
        assertEq(balancesAPost.debt, _qDebtA);

        assertEq(
            balancesBPost.collateral,
            _qCollateralB + amountsCollateral[vaultNumber]
        );
        assertEq(balancesBPost.debt, _qDebtB - amountsDebt[vaultNumber]);

        logMulticallStatuses(multicallStatuses);

        // only the second bid should be paid
        assertEq(
            balanceProtocolPost - balanceProtocolPre,
            (bidInfos[1].bid * _feeSplitTokenVault) /
                _feeSplitPrecisionTokenVault
        );
    }

    function testLiquidateWrongPermission() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(searcherA);
        bidInfos[0] = makeBidInfo(15, _searcherAOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoSearcherContracts(vaultNumber, bidInfos);

        // wrong permisison key
        permission = abi.encode(address(0));

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
        );

        assertEq(
            MyToken(tokensCollateral[vaultNumber]).balanceOf(
                address(searcherA)
            ),
            _qCollateralA
        );
        assertEq(
            MyToken(tokensDebt[vaultNumber]).balanceOf(address(searcherA)),
            _qDebtA
        );

        assertFailedExternal(multicallStatuses[0], "InvalidLiquidation()");
    }

    function testLiquidateMismatchedBid() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);
        uint256[] memory validUntils = new uint256[](1);
        uint256[] memory searcherSks = new uint256[](1);

        contracts[0] = address(searcherA);
        bidInfos[0] = makeBidInfo(15, _searcherAOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoSearcherContracts(vaultNumber, bidInfos);

        // mismatched bid--multicall expects higher bid than what is paid out by the searcher
        bidInfos[0].bid = bidInfos[0].bid + 1;

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
        );

        assertEq(
            MyToken(tokensCollateral[vaultNumber]).balanceOf(
                address(searcherA)
            ),
            _qCollateralA
        );
        assertEq(
            MyToken(tokensDebt[vaultNumber]).balanceOf(address(searcherA)),
            _qDebtA
        );

        assertEq(multicallStatuses[0].externalSuccess, false);
        assertEq(multicallStatuses[0].multicallRevertReason, "invalid bid");
    }

    function extractBidAmounts(
        BidInfo[] memory bids
    ) public pure returns (uint256[] memory bidAmounts) {
        bidAmounts = new uint256[](bids.length);
        for (uint i = 0; i < bids.length; i++) {
            bidAmounts[i] = bids[i].bid;
        }
    }

    struct AccountBalance {
        uint256 collateral;
        uint256 debt;
    }

    function assertEqBalances(
        AccountBalance memory a,
        AccountBalance memory b
    ) internal {
        assertEq(a.collateral, b.collateral);
        assertEq(a.debt, b.debt);
    }

    function getBalances(
        address account,
        uint vaultNumber
    ) public view returns (AccountBalance memory) {
        return
            AccountBalance(
                MyToken(tokensCollateral[vaultNumber]).balanceOf(account),
                MyToken(tokensDebt[vaultNumber]).balanceOf(account)
            );
    }

    function testLiquidateLiquidationAdapter() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(liquidationAdapter);
        bidInfos[0] = makeBidInfo(15, _searcherAOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoLiquidationAdapter(vaultNumber, bidInfos);

        AccountBalance memory balancesPre = getBalances(
            _searcherAOwnerAddress,
            vaultNumber
        );
        uint256 balanceProtocolPre = address(tokenVault).balance;

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
        );

        uint256 balanceProtocolPost = address(tokenVault).balance;

        AccountBalance memory balancesPost = getBalances(
            _searcherAOwnerAddress,
            vaultNumber
        );

        assertEq(
            balancesPost.collateral,
            balancesPre.collateral + amountsCollateral[vaultNumber]
        );
        assertEq(
            balancesPost.debt,
            balancesPre.debt - amountsDebt[vaultNumber]
        );

        assertEq(multicallStatuses[0].externalSuccess, true);

        assertEq(
            balanceProtocolPost - balanceProtocolPre,
            (bidInfos[0].bid * _feeSplitTokenVault) /
                _feeSplitPrecisionTokenVault
        );
    }

    function testLiquidateLiquidationAdapterFailInvalidSignature() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(liquidationAdapter);
        bidInfos[0] = makeBidInfo(15, _searcherBOwnerSk);
        bidInfos[0].liquidator = _searcherAOwnerAddress;

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoLiquidationAdapter(vaultNumber, bidInfos);

        AccountBalance memory balancesPre = getBalances(
            _searcherAOwnerAddress,
            vaultNumber
        );
        uint256 balanceProtocolPre = address(tokenVault).balance;

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
        );

        AccountBalance memory balancesPost = getBalances(
            _searcherAOwnerAddress,
            vaultNumber
        );
        uint256 balanceProtocolPost = address(tokenVault).balance;

        assertEqBalances(balancesPost, balancesPre);
        assertEq(balanceProtocolPre, balanceProtocolPost);

        assertFailedExternal(
            multicallStatuses[0],
            "InvalidSearcherSignature()"
        );
    }

    function assertFailedExternal(
        MulticallStatus memory status,
        string memory reason
    ) internal {
        assertEq(status.externalSuccess, false);
        // assert the first four bytes of the result matches the keccak hash of the error message
        assertEq(
            abi.encodePacked(bytes4(status.externalResult)),
            keccakHash(reason)
        );
    }

    function testLiquidateLiquidationAdapterFailExpiredSignature() public {
        uint256 vaultNumber = 0;

        address[] memory contracts = new address[](1);
        BidInfo[] memory bidInfos = new BidInfo[](1);

        contracts[0] = address(liquidationAdapter);
        bidInfos[0] = makeBidInfo(15, _searcherAOwnerSk);
        bidInfos[0].validUntil = block.number - 1; // use old block number for the validUntil field

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoLiquidationAdapter(vaultNumber, bidInfos);

        AccountBalance memory balancesPre = getBalances(
            _searcherAOwnerAddress,
            vaultNumber
        );
        uint256 balanceProtocolPre = address(tokenVault).balance;

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
        );

        AccountBalance memory balancesPost = getBalances(
            _searcherAOwnerAddress,
            vaultNumber
        );
        uint256 balanceProtocolPost = address(tokenVault).balance;

        assertEqBalances(balancesPost, balancesPre);
        assertEq(balanceProtocolPre, balanceProtocolPost);
        assertFailedExternal(multicallStatuses[0], "ExpiredSignature()");
    }

    struct BidInfo {
        uint256 bid;
        uint256 validUntil;
        address liquidator;
        uint256 liquidatorSk;
    }

    function makeBidInfo(
        uint256 bid,
        uint256 liquidatorSk
    ) internal pure returns (BidInfo memory) {
        return
            BidInfo(
                bid,
                1_000_000_000_000,
                vm.addr(liquidatorSk),
                liquidatorSk
            );
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
        bidInfos[0] = makeBidInfo(15, _searcherAOwnerSk);
        bidInfos[1] = makeBidInfo(10, _searcherBOwnerSk);

        (
            bytes memory permission,
            bytes[] memory data
        ) = getMulticallInfoLiquidationAdapter(vaultNumber, bidInfos);

        AccountBalance memory balancesAPre = getBalances(
            _searcherAOwnerAddress,
            vaultNumber
        );
        AccountBalance memory balancesBPre = getBalances(
            _searcherBOwnerAddress,
            vaultNumber
        );

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            extractBidAmounts(bidInfos)
        );

        AccountBalance memory balancesAPost = getBalances(
            _searcherAOwnerAddress,
            vaultNumber
        );
        AccountBalance memory balancesBPost = getBalances(
            _searcherBOwnerAddress,
            vaultNumber
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

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
import "solidity-bytes-utils/contracts/BytesLib.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";

import "openzeppelin-contracts/contracts/utils/Strings.sol";

import "../src/Errors.sol";

import "./helpers/ErrorChecks.sol";
import "./helpers/Signatures.sol";

contract PERVaultTest is Test, Signatures, ErrorChecks {
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

    uint256 _q1Depositor;
    uint256 _q2Depositor;
    uint256 _q1A;
    uint256 _q2A;
    uint256 _q1B;
    uint256 _q2B;
    uint256 _q2TokenVault;
    uint256 _q1Vault0;
    uint256 _q2Vault0;
    uint256 _q1Vault1;
    uint256 _q2Vault1;

    int64 _token2PriceLiqPermissionlessVault0;
    int64 _token2PriceLiqPERVault0;
    int64 _token2PriceLiqPermissionlessVault1;
    int64 _token2PriceLiqPERVault1;

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
        int32 token1Expo = 0;

        int64 token2Price = 100;
        uint64 token2Conf = 1;
        int32 token2Expo = 0;

        uint64 publishTime = 1_000_000;
        uint64 prevPublishTime = 0;

        vm.warp(publishTime);
        bytes memory token1UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken1,
            token1Price,
            token1Conf,
            token1Expo,
            token1Price,
            token1Conf,
            publishTime,
            prevPublishTime
        );
        bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken2,
            token2Price,
            token2Conf,
            token2Expo,
            token2Price,
            token2Conf,
            publishTime,
            prevPublishTime
        );

        bytes[] memory updateData = new bytes[](2);

        updateData[0] = token1UpdateData;
        updateData[1] = token2UpdateData;

        mockPyth.updatePriceFeeds(updateData);

        // create vault 0
        _q1Vault0 = 100;
        _q2Vault0 = 80;
        uint256 minCollatPERVault0 = 110 * healthPrecision;
        uint256 minCollatPermissionlessVault0 = 100 * healthPrecision;
        vm.prank(_depositor, _depositor);
        token1.approve(address(tokenVault), _q1Vault0);
        vm.prank(_depositor, _depositor);
        tokenVault.createVault(
            address(token1),
            address(token2),
            _q1Vault0,
            _q2Vault0,
            minCollatPERVault0,
            minCollatPermissionlessVault0,
            _idToken1,
            _idToken2,
            new bytes[](0)
        );
        _q1Depositor -= _q1Vault0;
        _q2Depositor += _q2Vault0;

        // create vault 1
        _q1Vault1 = 200;
        _q2Vault1 = 150;
        uint256 minCollatPERVault1 = 110 * healthPrecision;
        uint256 minCollatPermissionlessVault1 = 100 * healthPrecision;
        vm.prank(_depositor, _depositor);
        token1.approve(address(tokenVault), _q1Vault1);
        vm.prank(_depositor, _depositor);
        tokenVault.createVault(
            address(token1),
            address(token2),
            _q1Vault1,
            _q2Vault1,
            minCollatPERVault1,
            minCollatPermissionlessVault1,
            _idToken1,
            _idToken2,
            new bytes[](0)
        );
        _q1Depositor -= _q1Vault0;
        _q2Depositor += _q2Vault0;

        _token2PriceLiqPermissionlessVault0 = int64(
            uint64(
                (_q1Vault0 *
                    uint256(uint64(token1Price)) *
                    100 *
                    healthPrecision) /
                    (_q2Vault0 * minCollatPermissionlessVault0) +
                    1
            )
        );
        _token2PriceLiqPERVault0 = int64(
            uint64(
                (_q1Vault0 *
                    uint256(uint64(token1Price)) *
                    100 *
                    healthPrecision) /
                    (_q2Vault0 * minCollatPERVault0) +
                    1
            )
        );
        _token2PriceLiqPermissionlessVault1 = int64(
            uint64(
                (_q1Vault1 *
                    uint256(uint64(token1Price)) *
                    100 *
                    healthPrecision) /
                    (_q2Vault1 * minCollatPermissionlessVault1) +
                    1
            )
        );
        _token2PriceLiqPERVault1 = int64(
            uint64(
                (_q1Vault1 *
                    uint256(uint64(token1Price)) *
                    100 *
                    healthPrecision) /
                    (_q2Vault1 * minCollatPERVault1) +
                    1
            )
        );

        // fund searcher A and searcher B
        vm.deal(address(searcherA), 1 ether);
        vm.deal(address(searcherB), 1 ether);

        // mint tokens to searcher A wallet
        token2.mint(address(_searcherAOwnerAddress), _q2Vault0);
        // create allowance for liquidation adapter (token 2)
        vm.prank(_searcherAOwnerAddress, _searcherAOwnerAddress);
        token2.approve(address(liquidationAdapter), _q2Vault0);
        // deposit eth into the weth contract
        vm.deal(_searcherAOwnerAddress, 888 ether);
        vm.prank(_searcherAOwnerAddress, _searcherAOwnerAddress);
        weth.deposit{value: 888 ether}();
        // create allowance for liquidation adapter (weth)
        vm.prank(_searcherAOwnerAddress, _searcherAOwnerAddress);
        weth.approve(address(liquidationAdapter), 888 ether);

        // mint tokens to searcher B wallet
        token2.mint(address(_searcherBOwnerAddress), _q2Vault0);
        // create allowance for liquidation adapter (token 2)
        vm.prank(_searcherBOwnerAddress, _searcherBOwnerAddress);
        token2.approve(address(liquidationAdapter), _q2Vault0);
        // deposit eth into the weth contract
        vm.deal(_searcherBOwnerAddress, 999 ether);
        vm.prank(_searcherBOwnerAddress, _searcherBOwnerAddress);
        weth.deposit{value: 999 ether}();
        // create allowance for liquidation adapter (weth)
        vm.prank(_searcherBOwnerAddress, _searcherBOwnerAddress);
        weth.approve(address(liquidationAdapter), 999 ether);

        // fast forward to enable price updates in the below tests
        vm.warp(publishTime + 100);
    }

    function testLiquidateNoPER() public {
        // test permissionless liquidation (success)
        // raise price of token 2 to make vault 0 undercollateralized
        bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken2,
            _token2PriceLiqPermissionlessVault0,
            1,
            0,
            _token2PriceLiqPermissionlessVault0,
            1,
            uint64(block.timestamp),
            0
        );
        bytes memory signatureSearcher;

        uint256 validUntil = 1_000_000_000_000;

        vm.prank(_searcherAOwnerAddress, _searcherAOwnerAddress);
        searcherA.doLiquidate(
            0,
            0,
            validUntil,
            token2UpdateData,
            signatureSearcher
        );
        assertEq(token1.balanceOf(address(searcherA)), _q1A + _q1Vault0);
        assertEq(token2.balanceOf(address(searcherA)), _q2A - _q2Vault0);
    }

    function testLiquidateNoPERFail() public {
        // test permissionless liquidation (failure)
        // raise price of token 2 to make vault 0 undercollateralized
        bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken2,
            _token2PriceLiqPERVault0,
            1,
            0,
            _token2PriceLiqPERVault0,
            1,
            uint64(block.timestamp),
            0
        );
        bytes memory signatureSearcher;

        uint256 validUntil = 1_000_000_000_000;

        vm.expectRevert(abi.encodeWithSelector(InvalidLiquidation.selector));
        vm.prank(_searcherAOwnerAddress, _searcherAOwnerAddress);
        searcherA.doLiquidate(
            0,
            0,
            validUntil,
            token2UpdateData,
            signatureSearcher
        );
    }

    function testLiquidateSingle() public {
        // test PER path liquidation (via multicall, per operator calls) with searcher contract
        uint256 bid = 15;
        uint256 validUntil = 1_000_000_000_000; // TODO: need a test for historical validUntil values
        uint256 vaultNumber = 0;

        vm.roll(2);

        // get permission key
        bytes memory permission = abi.encode(
            address(tokenVault),
            abi.encodePacked(vaultNumber)
        );

        // create searcher signature
        bytes memory signatureSearcher = createSearcherSignature(
            vaultNumber,
            bid,
            validUntil,
            _searcherAOwnerSk
        );

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        uint256[] memory bids = new uint256[](1);

        // raise price of token 2 to make vault 0 undercollateralized
        bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken2,
            _token2PriceLiqPERVault0,
            1,
            0,
            _token2PriceLiqPERVault0,
            1,
            uint64(block.timestamp),
            0
        );

        contracts[0] = address(searcherA);
        data[0] = abi.encodeWithSignature(
            "doLiquidate(uint256,uint256,uint256,bytes,bytes)",
            0,
            bid,
            validUntil,
            token2UpdateData,
            signatureSearcher
        );
        bids[0] = bid;

        uint256 balanceProtocolPre = address(tokenVault).balance;

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            bids
        );

        uint256 balanceProtocolPost = address(tokenVault).balance;

        assertEq(token1.balanceOf(address(searcherA)), _q1A + _q1Vault0);
        assertEq(token2.balanceOf(address(searcherA)), _q2A - _q2Vault0);

        assertEq(multicallStatuses[0].externalSuccess, true);

        assertEq(
            balanceProtocolPost - balanceProtocolPre,
            (bid * _feeSplitTokenVault) / _feeSplitPrecisionTokenVault
        );
    }

    function testLiquidateMultipleFailSecond() public {
        uint256 bid0 = 15;
        uint256 validUntil0 = 1_000_000_000_000;

        uint256 bid1 = 10;
        uint256 validUntil1 = 1_000_000_000_000;

        uint256 vaultNumber = 0;

        vm.roll(2);

        bytes memory permission = abi.encode(
            address(tokenVault),
            abi.encodePacked(vaultNumber)
        );

        bytes memory signatureSearcher0 = createSearcherSignature(
            vaultNumber,
            bid0,
            validUntil0,
            _searcherAOwnerSk
        );
        bytes memory signatureSearcher1 = createSearcherSignature(
            vaultNumber,
            bid1,
            validUntil1,
            _searcherBOwnerSk
        );

        address[] memory contracts = new address[](2);
        bytes[] memory data = new bytes[](2);
        uint256[] memory bids = new uint256[](2);

        bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken2,
            _token2PriceLiqPERVault0,
            1,
            0,
            _token2PriceLiqPERVault0,
            1,
            uint64(block.timestamp),
            0
        );

        contracts[0] = address(searcherA);
        contracts[1] = address(searcherB);

        data[0] = abi.encodeWithSignature(
            "doLiquidate(uint256,uint256,uint256,bytes,bytes)",
            0,
            bid0,
            validUntil0,
            token2UpdateData,
            signatureSearcher0
        );
        data[1] = abi.encodeWithSignature(
            "doLiquidate(uint256,uint256,uint256,bytes,bytes)",
            0,
            bid1,
            validUntil1,
            token2UpdateData,
            signatureSearcher1
        );

        bids[0] = bid0;
        bids[1] = bid1;

        uint256 balanceProtocolPre = address(tokenVault).balance;

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            bids
        );

        uint256 balanceProtocolPost = address(tokenVault).balance;

        assertEq(token1.balanceOf(address(searcherA)), _q1A + _q1Vault0);
        assertEq(token2.balanceOf(address(searcherA)), _q2A - _q2Vault0);

        assertEq(token1.balanceOf(address(searcherB)), _q1B);
        assertEq(token2.balanceOf(address(searcherB)), _q2B);

        console.log("Success");
        console.log(multicallStatuses[0].externalSuccess);
        console.log(multicallStatuses[1].externalSuccess);
        console.log("Result");
        console.logBytes(multicallStatuses[0].externalResult);
        console.logBytes(multicallStatuses[1].externalResult);
        console.log("Revert reason");
        console.log(multicallStatuses[0].multicallRevertReason);
        console.log(multicallStatuses[1].multicallRevertReason);

        // only the first bid should be paid
        assertEq(
            balanceProtocolPost - balanceProtocolPre,
            (bid0 * _feeSplitTokenVault) / _feeSplitPrecisionTokenVault
        );
    }

    function testLiquidateMultipleFailFirst() public {
        uint256 bid0 = 15;
        uint256 validUntil0 = 1_000_000_000_000;

        uint256 bid1 = 10;
        uint256 validUntil1 = 1_000_000_000_000;

        uint256 vaultNumber = 0;

        vm.roll(2);

        bytes memory permission = abi.encode(
            address(tokenVault),
            abi.encodePacked(vaultNumber)
        );

        bytes memory signatureSearcher0 = createSearcherSignature(
            vaultNumber,
            bid0,
            validUntil0,
            _searcherAOwnerSk
        );
        bytes memory signatureSearcher1 = createSearcherSignature(
            vaultNumber,
            bid1,
            validUntil1,
            _searcherBOwnerSk
        );

        address[] memory contracts = new address[](2);
        bytes[] memory data = new bytes[](2);
        uint256[] memory bids = new uint256[](2);

        bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken2,
            _token2PriceLiqPERVault0,
            1,
            0,
            _token2PriceLiqPERVault0,
            1,
            uint64(block.timestamp),
            0
        );

        contracts[0] = address(searcherA);
        contracts[1] = address(searcherB);

        data[0] = abi.encodeWithSignature(
            "doLiquidate(uint256,uint256,uint256,bytes,bytes)",
            0,
            bid0,
            validUntil0,
            token2UpdateData,
            signatureSearcher0
        );
        data[1] = abi.encodeWithSignature(
            "doLiquidate(uint256,uint256,uint256,bytes,bytes)",
            0,
            bid1,
            validUntil1,
            token2UpdateData,
            signatureSearcher1
        );

        bids[0] = bid0;
        bids[1] = bid1;

        uint256 balanceProtocolPre = address(tokenVault).balance;

        // drain searcherA contract of Eth, so that the first liquidation fails
        vm.prank(_searcherAOwnerAddress, _searcherAOwnerAddress);
        searcherA.withdrawEth(address(searcherA).balance);

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            bids
        );

        uint256 balanceProtocolPost = address(tokenVault).balance;

        assertEq(token1.balanceOf(address(searcherA)), _q1A);
        assertEq(token2.balanceOf(address(searcherA)), _q2A);

        assertEq(token1.balanceOf(address(searcherB)), _q1B + _q1Vault0);
        assertEq(token2.balanceOf(address(searcherB)), _q2B - _q2Vault0);

        console.log("Success");
        console.log(multicallStatuses[0].externalSuccess);
        console.log(multicallStatuses[1].externalSuccess);
        console.log("Result");
        console.logBytes(multicallStatuses[0].externalResult);
        console.logBytes(multicallStatuses[1].externalResult);
        console.log("Revert reason");
        console.log(multicallStatuses[0].multicallRevertReason);
        console.log(multicallStatuses[1].multicallRevertReason);

        // only the second bid should be paid
        assertEq(
            balanceProtocolPost - balanceProtocolPre,
            (bid1 * _feeSplitTokenVault) / _feeSplitPrecisionTokenVault
        );
    }

    function testLiquidateWrongPermission() public {
        uint256 bid = 15;
        uint256 validUntil = 1_000_000_000_000;
        uint256 vaultNumber = 0;

        vm.roll(2);

        // wrong permisison key
        bytes memory permission = abi.encode(
            address(0),
            abi.encodePacked(vaultNumber)
        );

        bytes memory signatureSearcher = createSearcherSignature(
            vaultNumber,
            bid,
            validUntil,
            _searcherAOwnerSk
        );

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        uint256[] memory bids = new uint256[](1);

        bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken2,
            _token2PriceLiqPERVault0,
            1,
            0,
            _token2PriceLiqPERVault0,
            1,
            uint64(block.timestamp),
            0
        );

        contracts[0] = address(searcherA);
        data[0] = abi.encodeWithSignature(
            "doLiquidate(uint256,uint256,uint256,bytes,bytes)",
            0,
            bid,
            validUntil,
            token2UpdateData,
            signatureSearcher
        );
        bids[0] = bid;

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            bids
        );

        assertEq(token1.balanceOf(address(searcherA)), _q1A);
        assertEq(token2.balanceOf(address(searcherA)), _q2A);

        assertEq(multicallStatuses[0].externalSuccess, false);
        assertEq(
            multicallStatuses[0].externalResult,
            keccakHash("InvalidLiquidation()")
        );
    }

    function testLiquidateMismatchedBid() public {
        uint256 bid = 15;
        uint256 validUntil = 1_000_000_000_000;
        uint256 vaultNumber = 0;

        vm.roll(2);

        bytes memory permission = abi.encode(
            address(tokenVault),
            abi.encodePacked(vaultNumber)
        );

        bytes memory signatureSearcher = createSearcherSignature(
            vaultNumber,
            bid,
            validUntil,
            _searcherAOwnerSk
        );

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        uint256[] memory bids = new uint256[](1);

        bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken2,
            _token2PriceLiqPERVault0,
            1,
            0,
            _token2PriceLiqPERVault0,
            1,
            uint64(block.timestamp),
            0
        );

        contracts[0] = address(searcherA);
        data[0] = abi.encodeWithSignature(
            "doLiquidate(uint256,uint256,uint256,bytes,bytes)",
            0,
            bid,
            validUntil,
            token2UpdateData,
            signatureSearcher
        );
        // mismatched bid--multicall expects higher bid than what is paid out by the searcher
        bids[0] = bid + 1;

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            bids
        );

        assertEq(token1.balanceOf(address(searcherA)), _q1A);
        assertEq(token2.balanceOf(address(searcherA)), _q2A);

        assertEq(multicallStatuses[0].externalSuccess, false);
        assertEq(multicallStatuses[0].multicallRevertReason, "invalid bid");
    }

    function testLiquidateLiquidationAdapter() public {
        uint256 bid = 15;
        uint256 validUntil = 1_000_000_000_000;
        uint256 vaultNumber = 0;

        vm.roll(2);

        bytes memory permission = abi.encode(
            address(tokenVault),
            abi.encodePacked(vaultNumber)
        );

        bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken2,
            _token2PriceLiqPERVault0,
            1,
            0,
            _token2PriceLiqPERVault0,
            1,
            uint64(block.timestamp),
            0
        );
        bytes[] memory updateDatas = new bytes[](1);
        updateDatas[0] = token2UpdateData;

        // create liquidation call params struct
        TokenQty[] memory repayTokens = new TokenQty[](1);
        repayTokens[0] = TokenQty(address(token2), _q2Vault0);
        TokenQty[] memory expectedReceiptTokens = new TokenQty[](1);
        expectedReceiptTokens[0] = TokenQty(address(token1), _q1Vault0);
        address liquidator = _searcherAOwnerAddress;
        uint256 liquidatorSk = _searcherAOwnerSk;
        address contractAddress = address(tokenVault);
        bytes memory calldataVault = abi.encodeWithSignature(
            "liquidateWithPriceUpdate(uint256,bytes[])",
            0,
            updateDatas
        );
        uint256 value = 0;

        bytes memory signatureLiquidator = createLiquidationSignature(
            repayTokens,
            expectedReceiptTokens,
            contractAddress,
            calldataVault,
            value,
            bid,
            validUntil,
            liquidatorSk
        );
        LiquidationCallParams
            memory liquidationCallParams = LiquidationCallParams(
                repayTokens,
                expectedReceiptTokens,
                liquidator,
                contractAddress,
                calldataVault,
                value,
                validUntil,
                bid,
                signatureLiquidator
            );

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        uint256[] memory bids = new uint256[](1);

        contracts[0] = address(liquidationAdapter);
        data[0] = abi.encodeWithSignature(
            "callLiquidation(((address,uint256)[],(address,uint256)[],address,address,bytes,uint256,uint256,uint256,bytes))",
            liquidationCallParams
        );
        bids[0] = bid;

        uint256 token1BalanceAPre = token1.balanceOf(_searcherAOwnerAddress);
        uint256 token2BalanceAPre = token2.balanceOf(_searcherAOwnerAddress);
        uint256 balanceProtocolPre = address(tokenVault).balance;

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            bids
        );

        uint256 balanceProtocolPost = address(tokenVault).balance;

        assertEq(
            token1.balanceOf(_searcherAOwnerAddress),
            token1BalanceAPre + _q1Vault0
        );
        assertEq(
            token2.balanceOf(_searcherAOwnerAddress),
            token2BalanceAPre - _q2Vault0
        );

        assertEq(multicallStatuses[0].externalSuccess, true);

        assertEq(
            balanceProtocolPost - balanceProtocolPre,
            (bid * _feeSplitTokenVault) / _feeSplitPrecisionTokenVault
        );
    }

    function testLiquidateLiquidationAdapterFailInvalidSignature() public {
        uint256 bid = 15;
        uint256 validUntil = 1_000_000_000_000;
        uint256 vaultNumber = 0;

        vm.roll(2);

        bytes memory permission = abi.encode(
            address(tokenVault),
            abi.encodePacked(vaultNumber)
        );

        bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken2,
            _token2PriceLiqPERVault0,
            1,
            0,
            _token2PriceLiqPERVault0,
            1,
            uint64(block.timestamp),
            0
        );
        bytes[] memory updateDatas = new bytes[](1);
        updateDatas[0] = token2UpdateData;

        // create liquidation call params struct
        TokenQty[] memory repayTokens = new TokenQty[](1);
        repayTokens[0] = TokenQty(address(token2), _q2Vault0);
        TokenQty[] memory expectedReceiptTokens = new TokenQty[](1);
        expectedReceiptTokens[0] = TokenQty(address(token1), _q1Vault0);
        address liquidator = _searcherAOwnerAddress;
        uint256 liquidatorSk = _searcherAOwnerSk;
        address contractAddress = address(tokenVault);
        bytes memory calldataVault = abi.encodeWithSignature(
            "liquidateWithPriceUpdate(uint256,bytes[])",
            0,
            updateDatas
        );
        uint256 value = 0;

        // create invalid signature by using the wrong value
        bytes memory signatureLiquidator = createLiquidationSignature(
            repayTokens,
            expectedReceiptTokens,
            contractAddress,
            calldataVault,
            value + 44,
            bid,
            validUntil,
            liquidatorSk
        );
        LiquidationCallParams
            memory liquidationCallParams = LiquidationCallParams(
                repayTokens,
                expectedReceiptTokens,
                liquidator,
                contractAddress,
                calldataVault,
                value,
                validUntil,
                bid,
                signatureLiquidator
            );

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        uint256[] memory bids = new uint256[](1);

        contracts[0] = address(liquidationAdapter);
        data[0] = abi.encodeWithSignature(
            "callLiquidation(((address,uint256)[],(address,uint256)[],address,address,bytes,uint256,uint256,uint256,bytes))",
            liquidationCallParams
        );
        bids[0] = bid;

        uint256 token1BalanceAPre = token1.balanceOf(_searcherAOwnerAddress);
        uint256 token2BalanceAPre = token2.balanceOf(_searcherAOwnerAddress);
        uint256 balanceProtocolPre = address(tokenVault).balance;

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            bids
        );

        assertEq(token1.balanceOf(_searcherAOwnerAddress), token1BalanceAPre);
        assertEq(token2.balanceOf(_searcherAOwnerAddress), token2BalanceAPre);
        assertEq(balanceProtocolPre, address(tokenVault).balance);

        assertEq(multicallStatuses[0].externalSuccess, false);
        assertEq(
            multicallStatuses[0].externalResult,
            keccakHash("InvalidSearcherSignature()")
        );
    }

    function testLiquidateLiquidationAdapterFailExpiredSignature() public {
        uint256 bid = 15;
        // use old block number for the validUntil value
        uint256 validUntil = block.number - 1;
        uint256 vaultNumber = 0;

        vm.roll(2);

        bytes memory permission = abi.encode(
            address(tokenVault),
            abi.encodePacked(vaultNumber)
        );

        bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken2,
            _token2PriceLiqPERVault0,
            1,
            0,
            _token2PriceLiqPERVault0,
            1,
            uint64(block.timestamp),
            0
        );
        bytes[] memory updateDatas = new bytes[](1);
        updateDatas[0] = token2UpdateData;

        // create liquidation call params struct
        TokenQty[] memory repayTokens = new TokenQty[](1);
        repayTokens[0] = TokenQty(address(token2), _q2Vault0);
        TokenQty[] memory expectedReceiptTokens = new TokenQty[](1);
        expectedReceiptTokens[0] = TokenQty(address(token1), _q1Vault0);
        address liquidator = _searcherAOwnerAddress;
        uint256 liquidatorSk = _searcherAOwnerSk;
        address contractAddress = address(tokenVault);
        bytes memory calldataVault = abi.encodeWithSignature(
            "liquidateWithPriceUpdate(uint256,bytes[])",
            0,
            updateDatas
        );
        uint256 value = 0;

        bytes memory signatureLiquidator = createLiquidationSignature(
            repayTokens,
            expectedReceiptTokens,
            contractAddress,
            calldataVault,
            value,
            bid,
            validUntil,
            liquidatorSk
        );
        LiquidationCallParams
            memory liquidationCallParams = LiquidationCallParams(
                repayTokens,
                expectedReceiptTokens,
                liquidator,
                contractAddress,
                calldataVault,
                value,
                validUntil,
                bid,
                signatureLiquidator
            );

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        uint256[] memory bids = new uint256[](1);

        contracts[0] = address(liquidationAdapter);
        data[0] = abi.encodeWithSignature(
            "callLiquidation(((address,uint256)[],(address,uint256)[],address,address,bytes,uint256,uint256,uint256,bytes))",
            liquidationCallParams
        );
        bids[0] = bid;

        uint256 token1BalanceAPre = token1.balanceOf(_searcherAOwnerAddress);
        uint256 token2BalanceAPre = token2.balanceOf(_searcherAOwnerAddress);
        uint256 balanceProtocolPre = address(tokenVault).balance;

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            bids
        );

        assertEq(token1.balanceOf(_searcherAOwnerAddress), token1BalanceAPre);
        assertEq(token2.balanceOf(_searcherAOwnerAddress), token2BalanceAPre);
        assertEq(balanceProtocolPre, address(tokenVault).balance);

        assertEq(multicallStatuses[0].externalSuccess, false);
        assertEq(
            multicallStatuses[0].externalResult,
            keccakHash("ExpiredSignature()")
        );
    }

    function testLiquidateLiquidationAdapterFailLiquidationCall() public {
        uint256 bid = 15;
        uint256 validUntil = 1_000_000_000_000;
        // use wrong vault number to trigger failed liquidation call
        uint256 vaultNumber = 44;

        vm.roll(2);

        bytes memory permission = abi.encode(
            address(tokenVault),
            abi.encodePacked(vaultNumber)
        );

        bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken2,
            _token2PriceLiqPERVault0,
            1,
            0,
            _token2PriceLiqPERVault0,
            1,
            uint64(block.timestamp),
            0
        );
        bytes[] memory updateDatas = new bytes[](1);
        updateDatas[0] = token2UpdateData;

        // create liquidation call params struct
        TokenQty[] memory repayTokens = new TokenQty[](1);
        repayTokens[0] = TokenQty(address(token2), _q2Vault0);
        TokenQty[] memory expectedReceiptTokens = new TokenQty[](1);
        expectedReceiptTokens[0] = TokenQty(address(token1), _q1Vault0);
        address liquidator = _searcherAOwnerAddress;
        uint256 liquidatorSk = _searcherAOwnerSk;
        address contractAddress = address(tokenVault);
        bytes memory calldataVault = abi.encodeWithSignature(
            "liquidateWithPriceUpdate(uint256,bytes[])",
            0,
            updateDatas
        );
        uint256 value = 0;

        bytes memory signatureLiquidator = createLiquidationSignature(
            repayTokens,
            expectedReceiptTokens,
            contractAddress,
            calldataVault,
            value,
            bid,
            validUntil,
            liquidatorSk
        );
        LiquidationCallParams
            memory liquidationCallParams = LiquidationCallParams(
                repayTokens,
                expectedReceiptTokens,
                liquidator,
                contractAddress,
                calldataVault,
                value,
                validUntil,
                bid,
                signatureLiquidator
            );

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        uint256[] memory bids = new uint256[](1);

        contracts[0] = address(liquidationAdapter);
        data[0] = abi.encodeWithSignature(
            "callLiquidation(((address,uint256)[],(address,uint256)[],address,address,bytes,uint256,uint256,uint256,bytes))",
            liquidationCallParams
        );
        bids[0] = bid;

        uint256 token1BalanceAPre = token1.balanceOf(_searcherAOwnerAddress);
        uint256 token2BalanceAPre = token2.balanceOf(_searcherAOwnerAddress);
        uint256 balanceProtocolPre = address(tokenVault).balance;

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            bids
        );

        assertEq(token1.balanceOf(_searcherAOwnerAddress), token1BalanceAPre);
        assertEq(token2.balanceOf(_searcherAOwnerAddress), token2BalanceAPre);
        assertEq(balanceProtocolPre, address(tokenVault).balance);

        assertEq(multicallStatuses[0].externalSuccess, false);
        // assert the first four bytes of the result matches the keccak hash of the error message
        assertEq(
            BytesLib.slice(multicallStatuses[0].externalResult, 0, 4),
            keccakHash("LiquidationCallFailed(string)")
        );
    }

    function testLiquidateLiquidationAdapterFailExpectedReceiptTokenMismatch()
        public
    {
        uint256 bid = 15;
        uint256 validUntil = 1_000_000_000_000;
        uint256 vaultNumber = 0;

        vm.roll(2);

        bytes memory permission = abi.encode(
            address(tokenVault),
            abi.encodePacked(vaultNumber)
        );

        bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken2,
            _token2PriceLiqPERVault0,
            1,
            0,
            _token2PriceLiqPERVault0,
            1,
            uint64(block.timestamp),
            0
        );
        bytes[] memory updateDatas = new bytes[](1);
        updateDatas[0] = token2UpdateData;

        // create liquidation call params struct
        TokenQty[] memory repayTokens = new TokenQty[](1);
        repayTokens[0] = TokenQty(address(token2), _q2Vault0);
        TokenQty[] memory expectedReceiptTokens = new TokenQty[](1);
        // create a mismatch in the expected receipt token to induce failure
        expectedReceiptTokens[0] = TokenQty(address(token1), _q1Vault0 + 444);
        address liquidator = _searcherAOwnerAddress;
        uint256 liquidatorSk = _searcherAOwnerSk;
        address contractAddress = address(tokenVault);
        bytes memory calldataVault = abi.encodeWithSignature(
            "liquidateWithPriceUpdate(uint256,bytes[])",
            0,
            updateDatas
        );
        uint256 value = 0;

        bytes memory signatureLiquidator = createLiquidationSignature(
            repayTokens,
            expectedReceiptTokens,
            contractAddress,
            calldataVault,
            value,
            bid,
            validUntil,
            liquidatorSk
        );
        LiquidationCallParams
            memory liquidationCallParams = LiquidationCallParams(
                repayTokens,
                expectedReceiptTokens,
                liquidator,
                contractAddress,
                calldataVault,
                value,
                validUntil,
                bid,
                signatureLiquidator
            );

        address[] memory contracts = new address[](1);
        bytes[] memory data = new bytes[](1);
        uint256[] memory bids = new uint256[](1);

        contracts[0] = address(liquidationAdapter);
        data[0] = abi.encodeWithSignature(
            "callLiquidation(((address,uint256)[],(address,uint256)[],address,address,bytes,uint256,uint256,uint256,bytes))",
            liquidationCallParams
        );
        bids[0] = bid;

        uint256 token1BalanceAPre = token1.balanceOf(_searcherAOwnerAddress);
        uint256 token2BalanceAPre = token2.balanceOf(_searcherAOwnerAddress);
        uint256 balanceProtocolPre = address(tokenVault).balance;

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        MulticallStatus[] memory multicallStatuses = multicall.multicall(
            permission,
            contracts,
            data,
            bids
        );

        assertEq(token1.balanceOf(_searcherAOwnerAddress), token1BalanceAPre);
        assertEq(token2.balanceOf(_searcherAOwnerAddress), token2BalanceAPre);
        assertEq(balanceProtocolPre, address(tokenVault).balance);

        assertEq(multicallStatuses[0].externalSuccess, false);
        assertEq(
            multicallStatuses[0].externalResult,
            keccakHash("InsufficientTokenReceived()")
        );
    }
}

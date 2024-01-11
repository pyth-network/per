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
import "../src/Structs.sol";

import "@pythnetwork/pyth-sdk-solidity/MockPyth.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";

import "openzeppelin-contracts/contracts/utils/Strings.sol";

import "../src/Errors.sol";

import "./helpers/Signatures.sol";

contract PERVaultTest is Test, Signatures {
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
        vm.prank(_depositor, _depositor);
        token1.approve(address(tokenVault), _q1Vault0);
        vm.prank(_depositor, _depositor);
        tokenVault.createVault(
            address(token1),
            address(token2),
            _q1Vault0,
            _q2Vault0,
            110 * healthPrecision,
            100 * healthPrecision,
            _idToken1,
            _idToken2
        );
        _q1Depositor -= _q1Vault0;
        _q2Depositor += _q2Vault0;

        // create vault 1
        _q1Vault1 = 200;
        _q2Vault1 = 150;
        vm.prank(_depositor, _depositor);
        token1.approve(address(tokenVault), _q1Vault1);
        vm.prank(_depositor, _depositor);
        tokenVault.createVault(
            address(token1),
            address(token2),
            _q1Vault1,
            _q2Vault1,
            110 * healthPrecision,
            100 * healthPrecision,
            _idToken1,
            _idToken2
        );
        _q1Depositor -= _q1Vault0;
        _q2Depositor += _q2Vault0;

        // fund searcher A and searcher B
        vm.deal(address(searcherA), 1 ether);
        vm.deal(address(searcherB), 1 ether);

        // fast forward to enable price updates in the below tests
        vm.warp(publishTime + 100);
    }

    function testLiquidate() public {
        // test slow path liquidation
        // raise price of token 2 to make vault 0 undercollateralized, delayed oracle feed
        bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken2,
            200,
            1,
            0,
            200,
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

    function testLiquidatePERSingleContract() public {
        // test PER path liquidation (via multicall, per operator calls) with searcher contract
        uint256 bid = 15;
        uint256 validUntil = 1_000_000_000_000; // TODO: need a test for historical validUntil values

        vm.roll(2);

        uint256 vaultNumber = 0;

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
        address[] memory protocols = new address[](1);

        // raise price of token 2 to make vault 0 undercollateralized, fast oracle feed
        bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(
            _idToken2,
            200,
            1,
            0,
            200,
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
        protocols[0] = address(tokenVault);

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

        console.log("Success");
        console.log(multicallStatuses[0].externalSuccess);
        console.log("Result");
        console.logBytes(multicallStatuses[0].externalResult);
        console.log("Revert reason");
        console.log(multicallStatuses[0].multicallRevertReason);

        assertEq(
            balanceProtocolPost - balanceProtocolPre,
            (bid * _feeSplitTokenVault) / _feeSplitPrecisionTokenVault
        );
    }

    // function testLiquidateFastWrongContractAuction() public {
    //     // test fast path liquidation (via multicall, per operator calls)
    //     uint256 bid = 10;

    //     uint256 vaultNumber = 0;

    //     // create searcher signature
    //     bytes memory signatureSearcher = abi.encodePacked(vaultNumber, bid, block.number, _searcherAOwnerSk);

    //     // create PER signature, for the wrong contract address
    //     bytes memory signaturePer = createPerSignature(_signaturePerVersionNumber, address(4444), block.number, _perOperatorSk);

    //     address[] memory contracts = new address[](1);
    //     bytes[] memory data = new bytes[](1);
    //     uint256[] memory bids = new uint256[](1);
    //     address[] memory protocols = new address[](1);

    //     // raise price of token 2 to make vault 0 undercollateralized, fast oracle feed
    //     bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(_idToken2, 200, 1, 0, 200, 1, uint64(block.timestamp), 0);

    //     contracts[0] = address(searcherA);
    //     data[0] = abi.encodeWithSignature("doLiquidatePER(bytes,uint256,bytes,uint256,bytes)", signaturePer, 0, signatureSearcher, bid, token2UpdateData);
    //     bids[0] = bid;
    //     protocols[0] = address(tokenVault);

    //     vm.prank(_perOperatorAddress, _perOperatorAddress);
    //     (,, string[] memory multicallRevertReasons) = multicall.multicall(contracts, data, bids, protocols);

    //     assertEq(token1.balanceOf(address(searcherA)), _q1A);
    //     assertEq(token2.balanceOf(address(searcherA)), _q2A);

    //     assertEq(multicallRevertReasons[0], "invalid signature"); // there should be a revert error msg bc the PER signature is invalid
    // }

    // function testLiquidateFastWrongFunctionSignature() public {
    //     // test fast path liquidation (via multicall, per operator calls)
    //     uint256 bid = 10;

    //     uint256 vaultNumber = 0;

    //     // create searcher signature
    //     bytes memory signatureSearcher = createSearcherSignature(vaultNumber, bid, block.number, _searcherAOwnerSk);

    //     // create PER signature
    //     bytes memory signaturePer = createPerSignature(_signaturePerVersionNumber, address(tokenVault), block.number, _perOperatorSk);

    //     address[] memory contracts = new address[](1);
    //     bytes[] memory data = new bytes[](1);
    //     uint256[] memory bids = new uint256[](1);
    //     address[] memory protocols = new address[](1);

    //     // raise price of token 2 to make vault 0 undercollateralized, fast oracle feed
    //     bytes memory token2UpdateData = mockPyth.createPriceFeedUpdateData(_idToken2, 200, 1, 0, 200, 1, uint64(block.timestamp), 0);

    //     contracts[0] = address(searcherA);
    //     data[0] = abi.encodeWithSignature("fakeFunctionSignature(bytes,uint256,bytes,uint256,bytes)", signaturePer, 0, signatureSearcher, bid, token2UpdateData);
    //     bids[0] = bid;
    //     protocols[0] = address(tokenVault);

    //     vm.prank(_perOperatorAddress, _perOperatorAddress);
    //     (bool[] memory externalSuccess, bytes[] memory externalResults, string[] memory multicallRevertReasons) = multicall.multicall(contracts, data, bids, protocols);

    //     assertEq(token1.balanceOf(address(searcherA)), _q1A);
    //     assertEq(token2.balanceOf(address(searcherA)), _q2A);

    //     console.logBytes(externalResults[0]);
    //     console.log("multi revert reason", multicallRevertReasons[0]);

    //     assert(!externalSuccess[0]);
    //     assertEq(externalResults[0], abi.encodePacked(hex"")); // there should be no external failure reason bc this function signature is invalid
    // }

    // function testLiquidateFastMultiple() public {
    //     // test fast path liquidation on multiple vaults
    //     uint256 bid0 = 10;
    //     uint256 bid1 = 20;

    //     uint256 vaultNumber0 = 0;
    //     uint256 vaultNumber1 = 1;

    //     // create searcher signature
    //     bytes memory signatureSearcher0 = createSearcherSignature(vaultNumber0, bid0, block.number, _searcherAOwnerSk);

    //     // create searcher signature
    //     bytes memory signatureSearcher1 = createSearcherSignature(vaultNumber1, bid1, block.number, _searcherBOwnerSk);

    //     // create PER signature
    //     bytes memory signaturePer = createPerSignature(_signaturePerVersionNumber, address(tokenVault), block.number, _perOperatorSk);

    //     bytes memory token2UpdateData0 = mockPyth.createPriceFeedUpdateData(_idToken2, 200, 1, 0, 200, 1, uint64(block.timestamp-1), 0);
    //     bytes memory token2UpdateData1 = mockPyth.createPriceFeedUpdateData(_idToken2, 220, 1, 0, 220, 1, uint64(block.timestamp), 0);

    //     address[] memory contracts = new address[](2);
    //     bytes[] memory data = new bytes[](2);
    //     uint256[] memory bids = new uint256[](2);
    //     address[] memory protocols = new address[](2);

    //     contracts[0] = address(searcherA);
    //     contracts[1] = address(searcherB);
    //     data[0] = abi.encodeWithSignature("doLiquidatePER(bytes,uint256,bytes,uint256,bytes)", signaturePer, 0, signatureSearcher0, bid0, token2UpdateData0);
    //     data[1] = abi.encodeWithSignature("doLiquidatePER(bytes,uint256,bytes,uint256,bytes)", signaturePer, 1, signatureSearcher1, bid1, token2UpdateData1);
    //     bids[0] = bid0;
    //     bids[1] = bid1;
    //     protocols[0] = address(tokenVault);
    //     protocols[1] = address(tokenVault);

    //     uint256 balanceProtocolPre = address(tokenVault).balance;

    //     vm.prank(_perOperatorAddress, _perOperatorAddress);
    //     multicall.multicall(contracts, data, bids, protocols);

    //     uint256 balanceProtocolPost = address(tokenVault).balance;

    //     uint256 token1AAfter = token1.balanceOf(address(searcherA));
    //     uint256 token2AAfter = token2.balanceOf(address(searcherA));
    //     assertEq(token1AAfter, _q1A + _q1Vault0);
    //     assertEq(token2AAfter, _q2A - _q2Vault0);

    //     uint256 token1BAfter = token1.balanceOf(address(searcherB));
    //     uint256 token2BAfter = token2.balanceOf(address(searcherB));
    //     assertEq(token1BAfter, _q1B + _q1Vault1);
    //     assertEq(token2BAfter, _q2B - _q2Vault1);

    //     assertEq(balanceProtocolPost - balanceProtocolPre, bid0 * _feeSplitTokenVault / _feeSplitPrecisionTokenVault + bid1 * _feeSplitTokenVault / _feeSplitPrecisionTokenVault);
    // }

    // function testLiquidateFastMultipleWithFail() public {
    //     // test fast path liquidation on multiple vaults, with the second one failing due to earlier tx in the block that recollateralizes the vault
    //     uint256 bid0 = 10;
    //     uint256 bid1 = 30;

    //     uint256 vaultNumber0 = 0;
    //     uint256 vaultNumber1 = 1;

    //     // create searcher signature
    //     bytes memory signatureSearcher0 = createSearcherSignature(vaultNumber0, bid0, block.number, _searcherAOwnerSk);

    //     // create searcher signature
    //     bytes memory signatureSearcher1 = createSearcherSignature(vaultNumber1, bid1, block.number, _searcherBOwnerSk);

    //     // create PER signature
    //     bytes memory signaturePer = createPerSignature(_signaturePerVersionNumber, address(tokenVault), block.number, _perOperatorSk);

    //     bytes memory token2UpdateData0 = mockPyth.createPriceFeedUpdateData(_idToken2, 200, 1, 0, 200, 1, uint64(block.timestamp-1), 0);
    //     bytes memory token2UpdateData1 = mockPyth.createPriceFeedUpdateData(_idToken2, 220, 1, 0, 200, 1, uint64(block.timestamp), 0);

    //     address[] memory contracts = new address[](2);
    //     bytes[] memory data = new bytes[](2);
    //     uint256[] memory bids = new uint256[](2);
    //     address[] memory protocols = new address[](2);

    //     contracts[0] = address(searcherA);
    //     contracts[1] = address(searcherB);
    //     data[0] = abi.encodeWithSignature("doLiquidatePER(bytes,uint256,bytes,uint256,bytes)", signaturePer, 0, signatureSearcher0, bid0, token2UpdateData0);
    //     data[1] = abi.encodeWithSignature("doLiquidatePER(bytes,uint256,bytes,uint256,bytes)", signaturePer, 1, signatureSearcher1, bid1, token2UpdateData1);
    //     bids[0] = bid0;
    //     bids[1] = bid1;
    //     protocols[0] = address(tokenVault);
    //     protocols[1] = address(tokenVault);

    //     // frontrun in the block with an update to vault 1
    //     int256 deltaCollateral = int256(_q1Vault1 / 2);
    //     int256 deltaDebt = -1 * int256(_q2Vault1 / 2);
    //     vm.prank(_depositor, _depositor);
    //     token1.approve(address(tokenVault), stdMath.abs(deltaCollateral));
    //     vm.prank(_depositor, _depositor);
    //     token2.approve(address(tokenVault), stdMath.abs(deltaDebt));
    //     vm.prank(_depositor, _depositor);
    //     tokenVault.updateVault(1, deltaCollateral, deltaDebt);

    //     vm.prank(_perOperatorAddress, _perOperatorAddress);
    //     (bool[] memory externalSuccess, bytes[] memory externalResults, string[] memory multicallRevertReasons) = multicall.multicall(contracts, data, bids, protocols);

    //     assertEq(token1.balanceOf(address(searcherA)), _q1A + _q1Vault0);
    //     assertEq(token2.balanceOf(address(searcherA)), _q2A - _q2Vault0);

    //     assertEq(token1.balanceOf(address(searcherB)), _q1B);
    //     assertEq(token2.balanceOf(address(searcherB)), _q2B);

    //     assert(externalSuccess[0]);
    //     assert(!externalSuccess[1]); // this should be false bc searcher contract call failed

    //     assertEq(externalResults[0], abi.encodePacked(hex""));
    //     assertNotEq0(externalResults[1], abi.encodePacked(hex"")); // there should be a revert error code bc searcher contract call failed

    //     assertEq(multicallRevertReasons[0], "");
    //     assertEq(multicallRevertReasons[1], "");
    // }

    // function testLiquidateFastMultipleWithSecondFalseBid() public {
    //     // test fast path liquidation on multiple vaults, with the second one failing due to searcher not meeting bid condition
    //     uint256 bid0 = 10;
    //     uint256 bid1 = 30;

    //     uint256 vaultNumber0 = 0;
    //     uint256 vaultNumber1 = 1;

    //     // create searcher signature
    //     bytes memory signatureSearcher0 = createSearcherSignature(vaultNumber0, bid0, block.number, _searcherAOwnerSk);

    //     // create searcher signature
    //     bytes memory signatureSearcher1 = createSearcherSignature(vaultNumber1, bid1, block.number, _searcherBOwnerSk);

    //     // create PER signature
    //     bytes memory signaturePer = createPerSignature(_signaturePerVersionNumber, address(tokenVault), block.number, _perOperatorSk);

    //     bytes memory token2UpdateData0 = mockPyth.createPriceFeedUpdateData(_idToken2, 200, 1, 0, 200, 1, uint64(block.timestamp-1), 0);
    //     bytes memory token2UpdateData1 = mockPyth.createPriceFeedUpdateData(_idToken2, 200, 1, 0, 200, 1, uint64(block.timestamp), 0);

    //     address[] memory contracts = new address[](2);
    //     bytes[] memory data = new bytes[](2);
    //     uint256[] memory bids = new uint256[](2);
    //     address[] memory protocols = new address[](2);

    //     contracts[0] = address(searcherA);
    //     contracts[1] = address(searcherB);
    //     data[0] = abi.encodeWithSignature("doLiquidatePER(bytes,uint256,bytes,uint256,bytes)", signaturePer, 0, signatureSearcher0, bid0, token2UpdateData0);
    //     data[1] = abi.encodeWithSignature("doLiquidatePER(bytes,uint256,bytes,uint256,bytes)", signaturePer, 1, signatureSearcher1, bid1, token2UpdateData1);
    //     bids[0] = bid0;
    //     bids[1] = bid1+1; // actual promised bid was 1 wei higher than what searcher pays--should fail
    //     protocols[0] = address(tokenVault);
    //     protocols[1] = address(tokenVault);

    //     vm.prank(_perOperatorAddress, _perOperatorAddress);
    //     (, bytes[] memory externalResults, string[] memory multicallRevertReasons) = multicall.multicall(contracts, data, bids, protocols);

    //     uint256[] memory tokensAfter = new uint256[](4);
    //     tokensAfter[0] = token1.balanceOf(address(searcherA));
    //     tokensAfter[1] = token2.balanceOf(address(searcherA));
    //     tokensAfter[2] = token1.balanceOf(address(searcherB));
    //     tokensAfter[3] = token2.balanceOf(address(searcherB));

    //     assertEq(tokensAfter[0], _q1A + _q1Vault0);
    //     assertEq(tokensAfter[1], _q2A - _q2Vault0);

    //     assertEq(tokensAfter[2], _q1B);
    //     assertEq(tokensAfter[3], _q2B);

    //     assertEq(externalResults[0], abi.encodePacked(hex""));
    //     assertEq(externalResults[1], abi.encodePacked(hex""));

    //     assertEq(multicallRevertReasons[0], "");
    //     assertEq(multicallRevertReasons[1], "invalid bid"); // searcher B's tx should fail bc payment amount doesn't match bid
    // }

    // function testLiquidateFastInputFromEnvironVars() public {
    //     // test fast path liquidation with arbitrary calls, checking expected behavior
    //     // use environment variables to store the relevant inputs and expected outputs
    //     string memory delimiter = ",";

    //     // read in bundle contracts
    //     string memory keyContracts = "PERBUNDLE_contracts";
    //     address[] memory contracts = vm.envAddress(keyContracts, delimiter);

    //     // read in bundle calldata
    //     string memory keyData = "PERBUNDLE_data";
    //     bytes[] memory data = vm.envBytes(keyData, delimiter);

    //     // read in bundle bids
    //     string memory keyBids = "PERBUNDLE_bids";
    //     uint256[] memory bids = vm.envUint(keyBids, delimiter);

    //     // read in bundle protocols
    //     string memory keyProtocols = "PERBUNDLE_protocols";
    //     address[] memory protocols = vm.envAddress(keyProtocols, delimiter);

    //     // read in block number
    //     string memory keyBlockNumber = "PERBUNDLE_blockNumber";
    //     uint256 blockNumber = vm.envUint(keyBlockNumber);

    //     // roll to the block number specified in environ vars
    //     vm.roll(blockNumber);

    //     console.log("vault token 1 balance before:", token1.balanceOf(address(tokenVault)));
    //     console.log("vault token 2 balance before:", token2.balanceOf(address(tokenVault)));

    //     console.log("searcher A token 1 balance before:", token1.balanceOf(address(searcherA)));
    //     console.log("searcher A token 2 balance before:", token2.balanceOf(address(searcherA)));

    //     console.log("searcher B token 1 balance before:", token1.balanceOf(address(searcherB)));
    //     console.log("searcher B token 2 balance before:", token2.balanceOf(address(searcherB)));

    //     // now run multicall on the payload
    //     vm.prank(_perOperatorAddress, _perOperatorAddress);
    //     (bool[] memory externalSuccess, bytes[] memory externalResults, string[] memory multicallRevertReasons) = multicall.multicall(contracts, data, bids, protocols);

    //     console.log("vault token 1 balance after:", token1.balanceOf(address(tokenVault)));
    //     console.log("vault token 2 balance after:", token2.balanceOf(address(tokenVault)));

    //     console.log("searcher A token 1 balance after:", token1.balanceOf(address(searcherA)));
    //     console.log("searcher A token 2 balance after:", token2.balanceOf(address(searcherA)));

    //     console.log("searcher B token 1 balance after:", token1.balanceOf(address(searcherB)));
    //     console.log("searcher B token 2 balance after:", token2.balanceOf(address(searcherB)));

    //     for (uint i = 0; i < data.length; ++i) {
    //         console.log("success call %d", i);
    //         console.log(externalSuccess[i]);

    //         console.log("result call %d:", i);
    //         console.logBytes(externalResults[i]);

    //         console.log("revert reason call %d:", i);
    //         console.log(multicallRevertReasons[i]);
    //     }
    // }
}

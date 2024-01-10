// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Test, console2} from "forge-std/Test.sol";
import "../src/SigVerify.sol";
import "forge-std/console.sol";

import {Counter} from "../src/Counter.sol";
import {SearcherCounter} from "../src/SearcherCounter.sol";
import {PERMulticall} from "../src/PERMulticall.sol";
import {PERRegistry} from "../src/PERRegistry.sol";
import {PERSignatureValidation} from "../src/PERSignatureValidation.sol";
import "../src/Structs.sol";

import "openzeppelin-contracts/contracts/utils/Strings.sol";

import "../src/Errors.sol";

import "./helpers/Signatures.sol";

contract PERSimpleTest is Test, Signatures {
    Counter public counter;
    SearcherCounter public searcherA;
    SearcherCounter public searcherB;
    PERMulticall public multicall;
    PERRegistry public registry;
    PERSignatureValidation public signatureValidation;

    address _perOperatorAddress; uint256 _perOperatorSk; // address public immutable _perOperatorAddress = address(88);
    address _searcherAOwnerAddress; uint256 _searcherAOwnerSk;
    address _searcherBOwnerAddress; uint256 _searcherBOwnerSk;
    
    uint256 _defaultFeeSplitProtocol;
    uint256 _defaultFeeSplitPrecision;

    uint256 _signaturePerVersionNumber = 0;
    
    function setUp() public {
        // make PER operator wallet
        (_perOperatorAddress, _perOperatorSk) = makeAddrAndKey("perOperator");

        _defaultFeeSplitProtocol = 50;
        _defaultFeeSplitPrecision = 100;

        // instantiate registry contract
        vm.prank(_perOperatorAddress, _perOperatorAddress);
        registry = new PERRegistry(_defaultFeeSplitProtocol, _defaultFeeSplitPrecision);

        // instantiate PER signature validation contract
        vm.prank(_perOperatorAddress, _perOperatorAddress);
        signatureValidation = new PERSignatureValidation();

        // instantiate multicall contract with PER operator as sender/origin
        vm.prank(_perOperatorAddress, _perOperatorAddress);
        multicall = new PERMulticall(_perOperatorAddress, address(registry), address(signatureValidation));

        // make searcherA and searcherB wallets
        (_searcherAOwnerAddress, _searcherAOwnerSk) = makeAddrAndKey("searcherA");
        (_searcherBOwnerAddress, _searcherBOwnerSk) = makeAddrAndKey("searcherB");

        // instantiate counter contract
        counter = new Counter(address(multicall));

        // instantiate searcher A's contract with searcher A as sender/origin
        vm.prank(_searcherAOwnerAddress, _searcherAOwnerAddress);
        searcherA = new SearcherCounter(address(multicall), address(counter));

        // instantiate searcher B's contract with searcher B as sender/origin
        vm.prank(_searcherBOwnerAddress, _searcherBOwnerAddress);
        searcherB = new SearcherCounter(address(multicall), address(counter));

        counter.setNumber(0);

        // fund searcher A and searcher B contracts
        vm.deal(address(searcherA), 1 ether);
        vm.deal(address(searcherB), 1 ether);
    }

    function testIncrement() public {
        counter.increment(1);
        assertEq(counter.number(), 1);
        counter.increment(2);
        assertEq(counter.number(), 3);
    }

    function testFuzzSetNumber(uint256 x) public {
        counter.setNumber(x);
        assertEq(counter.number(), x);
    }

    function testIncrementFast() public {
        vm.prank(_perOperatorAddress, _perOperatorAddress);
        bytes memory emptySignature;
        counter.incrementFast(emptySignature, 1);
        assertEq(counter.number(), 1);
    }

    function testIncrementFastNotAsPER() public {
        vm.expectRevert(NotPEROperator.selector);
        bytes memory emptySignature;
        counter.incrementFast(emptySignature, 1);
    }

    function testSearcherAOwner() public {
        // test calling searcher contract by the owner's EOA directly
        uint256 inc = 3;

        uint256 bid = 0;

        bytes memory signature = createSearcherSignature(inc, bid, block.number, _searcherAOwnerSk);

        bytes memory emptySignature;
        vm.prank(_searcherAOwnerAddress, _searcherAOwnerAddress);
        searcherA.doIncrement(emptySignature, inc, signature, bid);
        assertEq(counter.number(), inc);
    }

    function multicallScenario(uint256 bid0, uint256 bid1) public {
        // test calling searcher contracts by going through the multicall via operator
        uint256 inc = 5;
        uint256 dec = 2;

        // create PER signature
        bytes memory signaturePer = createPerSignature(_signaturePerVersionNumber, address(counter), block.number, _perOperatorSk);

        // create searcher A increment message
        bytes memory signatureA = createSearcherSignature(inc, bid0, block.number, _searcherAOwnerSk);

        bytes memory searcherAData = abi.encodeWithSignature("doIncrement(bytes,uint256,bytes,uint256)", signaturePer, inc, signatureA, bid0);

        // create searcher B decrement message
        bytes memory signatureB = createSearcherSignature(dec, bid1, block.number, _searcherBOwnerSk);

        bytes memory searcherBData = abi.encodeWithSignature("doDecrement(bytes,uint256,bytes,uint256)", signaturePer, dec, signatureB, bid1);

        address[] memory contracts = new address[](2);
        bytes[] memory data = new bytes[](2);
        uint256[] memory bids = new uint256[](2);
        address[] memory protocols = new address[](2);
        
        contracts[0] = address(searcherA);
        contracts[1] = address(searcherB);
        data[0] = searcherAData;
        data[1] = searcherBData;
        bids[0] = bid0;
        bids[1] = bid1;
        protocols[0] = address(counter);
        protocols[1] = address(counter);

        vm.prank(_perOperatorAddress, _perOperatorAddress);
        multicall.multicall(contracts, data, bids, protocols);
        assertEq(counter.number(), inc-dec);
    }

    function testMulticallPreRegistration() public {
        uint256 balanceBefore = address(multicall).balance;
        uint256 bid0 = 2;
        uint256 bid1 = 1;

        multicallScenario(bid0, bid1);
        uint256 balanceAfter = address(multicall).balance;

        assertEq(balanceAfter - balanceBefore, bid0 + bid1);
    }

    function testMulticallPostRegistration() public {
        uint256 balanceBefore = address(multicall).balance;
        uint256 bid0 = 2;
        uint256 bid1 = 1;

        // register counter in PER
        counter.registerPER(address(registry));

        multicallScenario(bid0, bid1);
        uint256 balanceAfter = address(multicall).balance;

        uint256 feesProtocol = (bid0 * _defaultFeeSplitProtocol / _defaultFeeSplitPrecision) + (bid1 * _defaultFeeSplitProtocol / _defaultFeeSplitPrecision);

        assertEq(balanceAfter - balanceBefore, bid0 + bid1 - feesProtocol);
    }
}

// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {IERC20Errors} from "openzeppelin-contracts/contracts/interfaces/draft-IERC6093.sol";

import "../src/Errors.sol";
import "../src/Structs.sol";
import "../src/DummyExchange.sol";
import "../src/DummyExchangeUpgradable.sol";
import "../src/MyToken.sol";
import "./helpers/DummyExchangeHarness.sol";
import "permit2/interfaces/ISignatureTransfer.sol";
import "./PermitSignature.sol";

contract DummyExchangeUnitTest is Test, PermitSignature {
    DummyExchangeHarness dummyExchange;
    MyToken sellToken;
    MyToken buyToken;

    function setUp() public {
        setUpPermit2();
        dummyExchange = new DummyExchangeHarness(PERMIT2);
        sellToken = new MyToken("SellToken", "ST");
        buyToken = new MyToken("BuyToken", "BT");
    }

    function testTypeStrings() public {
        string memory dummyExchangeWitnessType = dummyExchange
            ._DUMMY_EXCHANGE_WITNESS_TYPE();
        string memory tokenAmountType = dummyExchange._TOKEN_AMOUNT_TYPE();
        // make sure tokenAmountType is at the end of dummyExchangeWitnessType
        for (uint i = 0; i < bytes(tokenAmountType).length; i++) {
            assertEq(
                bytes(dummyExchangeWitnessType)[
                    i +
                        bytes(dummyExchangeWitnessType).length -
                        bytes(tokenAmountType).length
                ],
                bytes(tokenAmountType)[i]
            );
        }
    }

    function makePermitFromSellTokens(
        TokenAmount[] memory sellTokens,
        DummyExchangeExecutionWitness memory witness,
        uint256 privateKey
    )
        public
        returns (
            ISignatureTransfer.PermitBatchTransferFrom memory permit,
            bytes memory signature
        )
    {
        ISignatureTransfer.TokenPermissions[]
            memory permitted = new ISignatureTransfer.TokenPermissions[](
                sellTokens.length
            );
        for (uint i = 0; i < sellTokens.length; i++) {
            permitted[i] = ISignatureTransfer.TokenPermissions({
                token: sellTokens[i].token,
                amount: sellTokens[i].amount
            });
        }
        permit = ISignatureTransfer.PermitBatchTransferFrom({
            permitted: permitted,
            nonce: 1000,
            deadline: block.timestamp + 1000
        });
        signature = getPermitBatchWitnessSignature(
            permit,
            privateKey,
            FULL_EXCHANGE_WITNESS_BATCH_TYPEHASH,
            dummyExchange.hash(witness),
            address(dummyExchange),
            EIP712Domain(PERMIT2).DOMAIN_SEPARATOR()
        );
    }

    function testCheckDuplicateTokensTokenAmount() public {
        TokenAmount[] memory tokens = new TokenAmount[](3);
        address token0 = makeAddr("token0");
        address token1 = makeAddr("token1");
        address token2 = makeAddr("token2");
        tokens[0] = TokenAmount(token0, 0);
        tokens[1] = TokenAmount(token1, 0);
        tokens[2] = TokenAmount(token2, 0);
        dummyExchange.exposed_checkDuplicateTokensTokenAmount(tokens);
        tokens[1] = TokenAmount(token2, 0);
        vm.expectRevert(DuplicateToken.selector);
        dummyExchange.exposed_checkDuplicateTokensTokenAmount(tokens);
    }

    function testCheckDuplicateTokensTokenPermissions() public {
        ISignatureTransfer.TokenPermissions[]
            memory tokens = new ISignatureTransfer.TokenPermissions[](3);
        address token0 = makeAddr("token0");
        address token1 = makeAddr("token1");
        address token2 = makeAddr("token2");
        tokens[0] = ISignatureTransfer.TokenPermissions(token0, 0);
        tokens[1] = ISignatureTransfer.TokenPermissions(token1, 0);
        tokens[2] = ISignatureTransfer.TokenPermissions(token2, 0);
        dummyExchange.exposed_checkDuplicateTokensTokenPermissions(tokens);
        tokens[1] = ISignatureTransfer.TokenPermissions(token2, 0);
        vm.expectRevert(DuplicateToken.selector);
        dummyExchange.exposed_checkDuplicateTokensTokenPermissions(tokens);
    }

    function prepare_buy_token(
        address owner,
        uint256 tokenAmount,
        uint256 balance
    ) internal returns (DummyExchangeExecutionWitness memory) {
        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        buyTokens[0] = TokenAmount(address(buyToken), tokenAmount);
        buyToken.mint(address(this), balance);
        buyToken.approve(address(dummyExchange), tokenAmount);
        DummyExchangeExecutionWitness
            memory witness = DummyExchangeExecutionWitness({
                buyTokens: buyTokens,
                owner: owner
            });
        return witness;
    }

    function testTransferBuyTokens(uint256 tokenAmount) public {
        address owner = makeAddr("owner");
        dummyExchange.exposed_transferBuyTokens(
            prepare_buy_token(owner, tokenAmount, tokenAmount)
        );
        assertEq(buyToken.balanceOf(address(dummyExchange)), 0);
        assertEq(buyToken.balanceOf(owner), tokenAmount);
        assertEq(buyToken.balanceOf(address(this)), 0);
    }

    function testRevertWhenInsufficientTokensInTransferBuyTokens(
        uint128 amount
    ) public {
        address owner = makeAddr("owner");
        uint256 tokenAmount = uint256(amount);
        DummyExchangeExecutionWitness memory witness = prepare_buy_token(
            owner,
            tokenAmount + 1,
            tokenAmount
        );
        vm.expectRevert(
            abi.encodeWithSelector(
                IERC20Errors.ERC20InsufficientBalance.selector,
                address(this),
                tokenAmount,
                tokenAmount + 1
            )
        );
        dummyExchange.exposed_transferBuyTokens(witness);
    }

    function prepare_sell_token(
        address owner,
        uint256 privateKey,
        uint256 tokenAmount,
        uint256 approveAmount,
        TokenAmount[] memory buyTokens
    )
        internal
        returns (
            ISignatureTransfer.PermitBatchTransferFrom memory,
            DummyExchangeExecutionWitness memory,
            bytes memory
        )
    {
        ISignatureTransfer.TokenPermissions[]
            memory sellTokens = new ISignatureTransfer.TokenPermissions[](1);
        sellTokens[0] = ISignatureTransfer.TokenPermissions(
            address(sellToken),
            tokenAmount
        );
        sellToken.mint(owner, tokenAmount);
        vm.prank(owner);
        sellToken.approve(address(PERMIT2), approveAmount);
        ISignatureTransfer.PermitBatchTransferFrom
            memory permit = ISignatureTransfer.PermitBatchTransferFrom({
                permitted: sellTokens,
                nonce: 1000,
                deadline: block.timestamp + 1000
            });
        DummyExchangeExecutionWitness
            memory witness = DummyExchangeExecutionWitness({
                buyTokens: buyTokens,
                owner: owner
            });
        bytes memory signature = getPermitBatchWitnessSignature(
            permit,
            privateKey,
            FULL_EXCHANGE_WITNESS_BATCH_TYPEHASH,
            dummyExchange.hash(witness),
            address(dummyExchange),
            EIP712Domain(PERMIT2).DOMAIN_SEPARATOR()
        );
        return (permit, witness, signature);
    }

    function testTransferSellTokens(uint256 tokenAmount) public {
        (address owner, uint256 privateKey) = makeAddrAndKey("owner");
        (
            ISignatureTransfer.PermitBatchTransferFrom memory permit,
            DummyExchangeExecutionWitness memory witness,
            bytes memory signature
        ) = prepare_sell_token(
                owner,
                privateKey,
                tokenAmount,
                tokenAmount,
                new TokenAmount[](0)
            );
        dummyExchange.exposed_transferSellTokens(permit, witness, signature);
        assertEq(sellToken.balanceOf(address(dummyExchange)), 0);
        assertEq(sellToken.balanceOf(owner), 0);
        assertEq(sellToken.balanceOf(address(this)), tokenAmount);
    }

    function testRevertWhenInsufficientTokensInTransferSellTokens(
        uint128 amount
    ) public {
        (address owner, uint256 privateKey) = makeAddrAndKey("owner");
        uint256 tokenAmount = uint256(amount);
        (
            ISignatureTransfer.PermitBatchTransferFrom memory permit,
            DummyExchangeExecutionWitness memory witness,
            bytes memory signature
        ) = prepare_sell_token(
                owner,
                privateKey,
                tokenAmount + 1,
                tokenAmount,
                new TokenAmount[](0)
            );
        vm.expectRevert("TRANSFER_FROM_FAILED");
        dummyExchange.exposed_transferSellTokens(permit, witness, signature);
    }

    function testExecuteExchange(uint256 buyAmount, uint256 sellAmount) public {
        (address owner, uint256 privateKey) = makeAddrAndKey("owner");
        DummyExchangeExecutionWitness memory buyWitness = prepare_buy_token(
            owner,
            buyAmount,
            buyAmount
        );
        (
            ISignatureTransfer.PermitBatchTransferFrom memory permit,
            DummyExchangeExecutionWitness memory witness,
            bytes memory signature
        ) = prepare_sell_token(
                owner,
                privateKey,
                sellAmount,
                sellAmount,
                buyWitness.buyTokens
            );
        DummyExchangeExecutionParams
            memory params = DummyExchangeExecutionParams({
                permit: permit,
                witness: witness
            });
        dummyExchange.executeExchange(params, signature);
        // assertEq(sellToken.balanceOf(address(dummyExchange)), 0);
        assertEq(buyToken.balanceOf(owner), buyAmount);
        assertEq(sellToken.balanceOf(address(this)), sellAmount);
    }
}

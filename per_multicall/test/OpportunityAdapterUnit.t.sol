// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import "forge-std/console.sol";
import "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import "../src/Errors.sol";
import "../src/Structs.sol";
import "../src/OpportunityAdapter.sol";
import "../src/OpportunityAdapterUpgradable.sol";
import "../src/MyToken.sol";
import "./helpers/Signatures/OpportunityAdapterSignature.sol";
import "./helpers/OpportunityAdapterHarness.sol";
import "permit2/interfaces/ISignatureTransfer.sol";
import "./PermitSignature.sol";

contract OpportunityAdapterUnitTest is
    Test,
    OpportunityAdapterSignature,
    PermitSignature
{
    OpportunityAdapterHarness opportunityAdapter;
    MyToken myToken;

    function setUp() public {
        opportunityAdapter = new OpportunityAdapterHarness();
        setUpPermit2();
        myToken = new MyToken("SellToken", "ST");
    }

    function testTypeStrings() public {
        string memory opportunityWitnessType = opportunityAdapter
            ._OPPORTUNITY_WITNESS_TYPE();
        string memory tokenAmountType = opportunityAdapter._TOKEN_AMOUNT_TYPE();
        // make sure tokenAmountType is at the end of opportunityWitnessType
        for (uint i = 0; i < bytes(tokenAmountType).length; i++) {
            assertEq(
                bytes(opportunityWitnessType)[
                    i +
                        bytes(opportunityWitnessType).length -
                        bytes(tokenAmountType).length
                ],
                bytes(tokenAmountType)[i]
            );
        }
    }

    function makePermitFromSellTokens(
        TokenAmount[] memory sellTokens,
        ExecutionWitness memory witness,
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
            FULL_WITNESS_BATCH_TYPEHASH,
            opportunityAdapter.hash(witness),
            address(opportunityAdapter),
            EIP712Domain(PERMIT2).DOMAIN_SEPARATOR()
        );
    }

    function testPrepareSellTokensRevokeAllowances(uint256 tokenAmount) public {
        TokenAmount[] memory sellTokens = new TokenAmount[](1);
        sellTokens[0] = TokenAmount(address(myToken), tokenAmount);
        (address executor, uint256 executorPrivateKey) = makeAddrAndKey(
            "executor"
        );
        myToken.mint(executor, tokenAmount);
        vm.prank(executor);
        myToken.approve(PERMIT2, tokenAmount);

        TokenAmount[] memory noTokens = new TokenAmount[](0);
        ExecutionWitness memory witness = ExecutionWitness({
            buyTokens: noTokens,
            executor: executor,
            targetContract: makeAddr("targetContract"),
            targetCalldata: "0x",
            targetCallValue: 0,
            bidAmount: 0
        });
        (
            ISignatureTransfer.PermitBatchTransferFrom memory permit,
            bytes memory signature
        ) = makePermitFromSellTokens(sellTokens, witness, executorPrivateKey);
        address targetContract = makeAddr("targetContract");

        opportunityAdapter.exposed_prepareSellTokens(
            permit,
            witness,
            signature
        );
        assertEq(myToken.balanceOf(address(opportunityAdapter)), tokenAmount);
        assertEq(
            myToken.allowance(address(opportunityAdapter), targetContract),
            tokenAmount
        );
        assertEq(myToken.balanceOf(executor), 0);

        opportunityAdapter.exposed_revokeAllowances(permit, targetContract);
        assertEq(
            myToken.allowance(address(opportunityAdapter), targetContract),
            0
        );
    }

    function testCheckDuplicateTokens() public {
        TokenAmount[] memory tokens = new TokenAmount[](3);
        address token0 = makeAddr("token0");
        address token1 = makeAddr("token1");
        address token2 = makeAddr("token2");
        tokens[0] = TokenAmount(token0, 0);
        tokens[1] = TokenAmount(token1, 0);
        tokens[2] = TokenAmount(token2, 0);
        opportunityAdapter.exposed_checkDuplicateTokens(tokens);
        tokens[1] = TokenAmount(token2, 0);
        vm.expectRevert(DuplicateToken.selector);
        opportunityAdapter.exposed_checkDuplicateTokens(tokens);
    }

    function testGetContractTokenBalances(uint256 tokenAmount) public {
        TokenAmount[] memory tokens = new TokenAmount[](2);
        tokens[0] = TokenAmount(address(myToken), 0);
        tokens[1] = TokenAmount(address(myToken), 0);
        myToken.mint(address(opportunityAdapter), tokenAmount);
        uint256[] memory balances = opportunityAdapter
            .exposed_getContractTokenBalances(tokens);
        assertEq(balances[0], tokenAmount);
        assertEq(balances[1], tokenAmount);
    }

    function testRevertWhenTokenDoesNotExistInGetContractTokenBalances()
        public
    {
        TokenAmount[] memory tokens = new TokenAmount[](2);
        tokens[0] = TokenAmount(address(myToken), 0);
        tokens[1] = TokenAmount(makeAddr("InvalidToken"), 0);
        vm.expectRevert();
        uint256[] memory balances = opportunityAdapter
            .exposed_getContractTokenBalances(tokens);
    }

    function testValidateAndTransferBuyTokens(uint256 tokenAmount) public {
        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        buyTokens[0] = TokenAmount(address(myToken), tokenAmount);
        address executor = makeAddr("executor");
        address targetContract = makeAddr("targetContract");
        uint256[] memory buyTokensBalancesBeforeCall = new uint256[](1);
        buyTokensBalancesBeforeCall[0] = 0;
        myToken.mint(address(opportunityAdapter), tokenAmount);
        opportunityAdapter.exposed_validateAndTransferBuyTokens(
            buyTokens,
            executor,
            buyTokensBalancesBeforeCall
        );
        assertEq(myToken.balanceOf(address(opportunityAdapter)), 0);
        assertEq(myToken.balanceOf(executor), tokenAmount);
    }

    function testRevertWhenInsufficientTokensInValidateAndTransferBuyTokens(
        uint128 tokenAmount
    ) public {
        // tokenAmount is uint128 to avoid overflow in the test
        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        buyTokens[0] = TokenAmount(address(myToken), tokenAmount);
        address executor = makeAddr("executor");
        address targetContract = makeAddr("targetContract");
        uint256[] memory buyTokensBalancesBeforeCall = new uint256[](1);
        buyTokensBalancesBeforeCall[0] = 1; // not all tokens were received because of the call
        myToken.mint(address(opportunityAdapter), tokenAmount);
        vm.expectRevert(InsufficientTokenReceived.selector);
        opportunityAdapter.exposed_validateAndTransferBuyTokens(
            buyTokens,
            executor,
            buyTokensBalancesBeforeCall
        );
    }
}

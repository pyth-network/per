// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import "src/express-relay/Errors.sol";
import "src/opportunity-adapter/OpportunityAdapter.sol";
import "./searcher-vault/Structs.sol";
import "./MyToken.sol";
import "./helpers/OpportunityAdapterHarness.sol";
import "permit2/interfaces/ISignatureTransfer.sol";
import "./PermitSignature.sol";

contract OpportunityAdapterUnitTest is
    Test,
    PermitSignature,
    IOpportunityAdapterFactory,
    OpportunityAdapterHasher
{
    struct Parameters {
        address expressRelay;
        address weth;
        address permit2;
        address owner;
    }

    Parameters public override parameters;

    OpportunityAdapterHarness opportunityAdapter;
    MyToken myToken;

    function setUp() public {
        setUpPermit2();
        parameters = Parameters({
            expressRelay: address(0),
            weth: address(0),
            permit2: PERMIT2,
            owner: makeAddr("executor")
        });
        opportunityAdapter = new OpportunityAdapterHarness();
        myToken = new MyToken("SellToken", "ST");
    }

    function testWithdrawEthOwner(uint256 amount) public {
        address owner = makeAddr("executor");
        vm.deal(address(opportunityAdapter), amount);

        vm.prank(owner);
        opportunityAdapter.withdrawEth();

        assertEq(address(opportunityAdapter).balance, 0);
        assertEq(owner.balance, amount);
    }

    function testRevertWithdrawEthNonOwner() public {
        address nonOwner = makeAddr("nonOwner");

        vm.prank(nonOwner);
        vm.expectRevert(OnlyOwnerCanCall.selector);
        opportunityAdapter.withdrawEth();
    }

    function testWithdrawTokenOwner(uint256 tokenAmount) public {
        address owner = makeAddr("executor");
        myToken.mint(address(opportunityAdapter), tokenAmount);

        vm.prank(owner);
        opportunityAdapter.withdrawToken(address(myToken));

        assertEq(myToken.balanceOf(address(opportunityAdapter)), 0);
        assertEq(myToken.balanceOf(owner), tokenAmount);
    }

    function testRevertWithdrawTokenNonOwner() public {
        address nonOwner = makeAddr("nonOwner");

        vm.prank(nonOwner);
        vm.expectRevert(OnlyOwnerCanCall.selector);
        opportunityAdapter.withdrawToken(address(myToken));
    }

    function testTypeStrings() public {
        string memory opportunityWitnessType = opportunityAdapter
            .getOpportunityWitnessType();
        string memory tokenAmountType = opportunityAdapter.getTokenAmountType();
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
            FULL_OPPORTUNITY_WITNESS_BATCH_TYPEHASH,
            hash(witness),
            address(opportunityAdapter),
            EIP712Domain(PERMIT2).DOMAIN_SEPARATOR()
        );
    }

    function testPrepareSellTokensApproveTokensRevokeAllowances(
        uint256 tokenAmount
    ) public {
        TokenAmount[] memory sellTokens = new TokenAmount[](1);
        sellTokens[0] = TokenAmount(address(myToken), tokenAmount);
        (address executor, uint256 executorPrivateKey) = makeAddrAndKey(
            "executor"
        );
        myToken.mint(executor, tokenAmount);
        vm.prank(executor);
        myToken.approve(PERMIT2, tokenAmount);

        TargetCall[] memory targetCalls = new TargetCall[](1);
        TokenToSend[] memory tokensToSend = new TokenToSend[](
            sellTokens.length
        );
        for (uint j = 0; j < sellTokens.length; j++) {
            tokensToSend[j] = TokenToSend(
                TokenAmount(sellTokens[j].token, sellTokens[j].amount),
                makeAddr("targetContract")
            );
        }
        targetCalls[0] = TargetCall(
            makeAddr("targetContract"),
            new bytes(0),
            0,
            tokensToSend
        );

        TokenAmount[] memory noTokens = new TokenAmount[](0);
        ExecutionWitness memory witness = ExecutionWitness({
            buyTokens: noTokens,
            executor: executor,
            targetCalls: targetCalls,
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
        assertEq(myToken.balanceOf(executor), 0);

        opportunityAdapter.exposed_approveTokens(tokensToSend);
        assertEq(
            myToken.allowance(address(opportunityAdapter), targetContract),
            tokenAmount
        );

        opportunityAdapter.exposed_revokeAllowances(tokensToSend);
        assertEq(
            myToken.allowance(address(opportunityAdapter), targetContract),
            0
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
        opportunityAdapter.exposed_checkDuplicateTokensTokenAmount(tokens);
        tokens[1] = TokenAmount(token2, 0);
        vm.expectRevert(DuplicateToken.selector);
        opportunityAdapter.exposed_checkDuplicateTokensTokenAmount(tokens);
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
        opportunityAdapter.exposed_checkDuplicateTokensTokenPermissions(tokens);
        tokens[1] = ISignatureTransfer.TokenPermissions(token2, 0);
        vm.expectRevert(DuplicateToken.selector);
        opportunityAdapter.exposed_checkDuplicateTokensTokenPermissions(tokens);
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

    function testValidateAndTransferBuyTokens(
        uint256 tokenAmount,
        uint256 excessTokenAmount
    ) public {
        vm.assume(tokenAmount <= type(uint256).max - excessTokenAmount); // to avoid overflow in the test
        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        buyTokens[0] = TokenAmount(address(myToken), tokenAmount);
        address executor = makeAddr("executor");
        address targetContract = makeAddr("targetContract");
        uint256[] memory buyTokensBalancesBeforeCall = new uint256[](1);
        buyTokensBalancesBeforeCall[0] = 0;
        myToken.mint(
            address(opportunityAdapter),
            tokenAmount + excessTokenAmount
        );
        opportunityAdapter.exposed_validateAndTransferBuyTokens(
            buyTokens,
            executor,
            buyTokensBalancesBeforeCall
        );
        assertEq(myToken.balanceOf(address(opportunityAdapter)), 0);
        assertEq(myToken.balanceOf(executor), tokenAmount + excessTokenAmount);
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

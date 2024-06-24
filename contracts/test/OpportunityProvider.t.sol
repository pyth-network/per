// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {IERC20Errors} from "openzeppelin-contracts/contracts/interfaces/draft-IERC6093.sol";

import "src/express-relay/ExpressRelayUpgradable.sol";
import "./opportunity-provider/Errors.sol";
import "./opportunity-provider/Structs.sol";
import "./opportunity-provider/OpportunityProvider.sol";
import "./MyToken.sol";
import "./WETH9.sol";
import "./helpers/OpportunityProviderHarness.sol";
import "permit2/interfaces/ISignatureTransfer.sol";
import "./PermitSignature.sol";
import {OpportunityAdapterFactory} from "src/opportunity-adapter/OpportunityAdapterFactory.sol";
import {OpportunityAdapterHasher} from "src/opportunity-adapter/OpportunityAdapterHasher.sol";
import {ExecutionWitness as AdapterExecutionWitness, ExecutionParams as AdapterExecutionParams, TokenAmount as AdapterTokenAmount} from "src/opportunity-adapter/Structs.sol";

contract OpportunityProviderUnitTest is
    Test,
    PermitSignature,
    OpportunityAdapterHasher
{
    OpportunityProviderHarness opportunityProvider;
    OpportunityAdapterFactory adapterFactory;
    ExpressRelayUpgradable expressRelay;

    WETH9 weth;
    MyToken sellToken;
    MyToken buyToken;

    uint256 constant feeSplitProtocolDefault = 50 * 10 ** 16;
    uint256 constant feeSplitRelayer = 10 ** 17;
    address admin;
    uint256 adminPrivateKey;
    address relayer;

    function setUpTokens() internal {
        buyToken = new MyToken("BuyToken", "BT");
        sellToken = new MyToken("SellToken", "ST");
        weth = new WETH9();
    }

    function setUpOpportunityAdapter() internal {
        adapterFactory = new OpportunityAdapterFactory(
            address(expressRelay),
            address(weth),
            PERMIT2
        );
    }

    function setUpOpportunityProvider() internal {
        opportunityProvider = new OpportunityProviderHarness(
            admin,
            address(expressRelay),
            PERMIT2
        );
    }

    function setUpExpressRelay() internal {
        (relayer, ) = makeAddrAndKey("relayer");
        (admin, adminPrivateKey) = makeAddrAndKey("admin");
        vm.prank(relayer);
        ExpressRelayUpgradable _expressRelay = new ExpressRelayUpgradable();
        // deploy proxy contract and point it to implementation
        ERC1967Proxy proxyExpressRelay = new ERC1967Proxy(
            address(_expressRelay),
            ""
        );
        expressRelay = ExpressRelayUpgradable(payable(proxyExpressRelay));
        expressRelay.initialize(
            relayer,
            address(this),
            relayer,
            feeSplitProtocolDefault,
            feeSplitRelayer
        );
    }

    function setUp() public {
        setUpTokens();
        setUpExpressRelay();
        setUpOpportunityProvider();
        setUpOpportunityAdapter();
        setUpPermit2();
    }

    function testTypeStrings() public view {
        string memory witnessType = opportunityProvider
            ._OPPORTUNITY_PROVIDER_WITNESS_TYPE();
        string memory tokenAmountType = opportunityProvider
            ._TOKEN_AMOUNT_TYPE();
        // make sure tokenAmountType is at the end of opportunityProviderWitnessType
        for (uint i = 0; i < bytes(tokenAmountType).length; i++) {
            assertEq(
                bytes(witnessType)[
                    i +
                        bytes(witnessType).length -
                        bytes(tokenAmountType).length
                ],
                bytes(tokenAmountType)[i]
            );
        }
    }

    function testCheckDuplicateTokensTokenAmount() public {
        TokenAmount[] memory tokens = new TokenAmount[](3);
        address token0 = makeAddr("token0");
        address token1 = makeAddr("token1");
        address token2 = makeAddr("token2");
        tokens[0] = TokenAmount(token0, 0);
        tokens[1] = TokenAmount(token1, 0);
        tokens[2] = TokenAmount(token2, 0);
        opportunityProvider.exposed_checkDuplicateTokensTokenAmount(tokens);
        tokens[1] = TokenAmount(token2, 0);
        vm.expectRevert(DuplicateToken.selector);
        opportunityProvider.exposed_checkDuplicateTokensTokenAmount(tokens);
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
        opportunityProvider.exposed_checkDuplicateTokensTokenPermissions(
            tokens
        );
        tokens[1] = ISignatureTransfer.TokenPermissions(token2, 0);
        vm.expectRevert(DuplicateToken.selector);
        opportunityProvider.exposed_checkDuplicateTokensTokenPermissions(
            tokens
        );
    }

    function prepareBuyTokens(
        address owner,
        uint256 tokenAmount,
        uint256 balance,
        address sellerAddress
    ) internal returns (ExecutionWitness memory) {
        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        buyTokens[0] = TokenAmount(address(buyToken), tokenAmount);
        buyToken.mint(sellerAddress, balance);
        vm.prank(sellerAddress);
        buyToken.approve(address(opportunityProvider), tokenAmount);
        ExecutionWitness memory witness = ExecutionWitness({
            buyTokens: buyTokens,
            owner: owner
        });
        return witness;
    }

    function testTransferBuyTokens(uint256 tokenAmount) public {
        address owner = makeAddr("owner");
        opportunityProvider.exposed_transferBuyTokens(
            prepareBuyTokens(owner, tokenAmount, tokenAmount, address(this))
        );
        assertEq(buyToken.balanceOf(address(opportunityProvider)), 0);
        assertEq(buyToken.balanceOf(owner), tokenAmount);
        assertEq(buyToken.balanceOf(address(this)), 0);
    }

    function testRevertWhenInsufficientTokensInTransferBuyTokens(
        uint128 amount
    ) public {
        address owner = makeAddr("owner");
        uint256 tokenAmount = uint256(amount);
        ExecutionWitness memory witness = prepareBuyTokens(
            owner,
            tokenAmount + 1,
            tokenAmount,
            address(this)
        );
        vm.expectRevert(
            abi.encodeWithSelector(
                IERC20Errors.ERC20InsufficientBalance.selector,
                address(this),
                tokenAmount,
                tokenAmount + 1
            )
        );
        opportunityProvider.exposed_transferBuyTokens(witness);
    }

    function prepareSellTokens(
        uint256 tokenAmount,
        uint256 approveAmount,
        TokenAmount[] memory buyTokens
    )
        internal
        returns (
            ISignatureTransfer.PermitBatchTransferFrom memory,
            ExecutionWitness memory,
            bytes memory
        )
    {
        ISignatureTransfer.TokenPermissions[]
            memory sellTokens = new ISignatureTransfer.TokenPermissions[](1);
        sellTokens[0] = ISignatureTransfer.TokenPermissions(
            address(sellToken),
            tokenAmount
        );
        sellToken.mint(admin, tokenAmount);
        vm.prank(admin);
        sellToken.approve(address(PERMIT2), approveAmount);
        ISignatureTransfer.PermitBatchTransferFrom
            memory permit = ISignatureTransfer.PermitBatchTransferFrom({
                permitted: sellTokens,
                nonce: 1000,
                deadline: block.timestamp + 1000
            });
        ExecutionWitness memory witness = ExecutionWitness({
            buyTokens: buyTokens,
            owner: admin
        });
        bytes memory signature = getPermitBatchWitnessSignature(
            permit,
            adminPrivateKey,
            FULL_OPPORTUNITY_PROVIDER_WITNESS_BATCH_TYPEHASH,
            opportunityProvider.hash(witness),
            address(opportunityProvider),
            EIP712Domain(PERMIT2).DOMAIN_SEPARATOR()
        );
        return (permit, witness, signature);
    }

    function testTransferSellTokens(uint256 tokenAmount) public {
        (
            ISignatureTransfer.PermitBatchTransferFrom memory permit,
            ExecutionWitness memory witness,
            bytes memory signature
        ) = prepareSellTokens(tokenAmount, tokenAmount, new TokenAmount[](0));
        opportunityProvider.exposed_transferSellTokens(
            permit,
            witness,
            signature
        );
        assertEq(sellToken.balanceOf(address(opportunityProvider)), 0);
        assertEq(sellToken.balanceOf(admin), 0);
        assertEq(sellToken.balanceOf(address(this)), tokenAmount);
    }

    function testRevertWhenInsufficientTokensInTransferSellTokens(
        uint128 amount
    ) public {
        uint256 tokenAmount = uint256(amount);
        (
            ISignatureTransfer.PermitBatchTransferFrom memory permit,
            ExecutionWitness memory witness,
            bytes memory signature
        ) = prepareSellTokens(
                tokenAmount + 1,
                tokenAmount,
                new TokenAmount[](0)
            );
        vm.expectRevert("TRANSFER_FROM_FAILED");
        opportunityProvider.exposed_transferSellTokens(
            permit,
            witness,
            signature
        );
    }

    function prepareTokensForOpportunityAdapter(
        ExecutionParams memory params,
        bytes memory providerSignature,
        uint256 bidAmount
    )
        internal
        returns (
            AdapterExecutionParams memory executionParams,
            bytes memory signature
        )
    {
        (address buyer, uint256 buyerPrivateKey) = makeAddrAndKey("buyer");
        uint256 tokenAmount = params.witness.buyTokens[0].amount;
        buyToken.mint(buyer, tokenAmount);
        vm.prank(buyer);
        buyToken.approve(PERMIT2, tokenAmount);

        ISignatureTransfer.TokenPermissions[]
            memory permitted = new ISignatureTransfer.TokenPermissions[](2);
        permitted[0] = ISignatureTransfer.TokenPermissions(
            address(buyToken),
            tokenAmount
        );
        permitted[1] = ISignatureTransfer.TokenPermissions(
            address(weth),
            bidAmount
        );
        ISignatureTransfer.PermitBatchTransferFrom
            memory permit = ISignatureTransfer.PermitBatchTransferFrom(
                permitted,
                100,
                block.timestamp + 1000
            );
        AdapterTokenAmount[] memory sellTokens = new AdapterTokenAmount[](1);
        sellTokens[0] = AdapterTokenAmount(
            address(sellToken),
            params.permit.permitted[0].amount
        );
        AdapterExecutionWitness memory witness = AdapterExecutionWitness(
            sellTokens,
            buyer,
            address(opportunityProvider),
            abi.encodeWithSelector(
                opportunityProvider.execute.selector,
                params,
                providerSignature
            ),
            0,
            bidAmount
        );

        vm.deal(buyer, 1 ether);
        vm.startPrank(buyer);
        weth.deposit{value: bidAmount}();
        weth.approve(PERMIT2, bidAmount);
        vm.stopPrank();

        executionParams = AdapterExecutionParams(permit, witness);
        signature = getPermitBatchWitnessSignature(
            permit,
            buyerPrivateKey,
            FULL_OPPORTUNITY_WITNESS_BATCH_TYPEHASH,
            hash(witness),
            adapterFactory.computeAddress(buyer),
            EIP712Domain(PERMIT2).DOMAIN_SEPARATOR()
        );
    }

    function testExecuteWithBidAndAdapter(
        uint256 buyAmount,
        uint256 sellAmount
    ) public {
        TokenAmount[] memory buyTokens = new TokenAmount[](1);
        buyTokens[0] = TokenAmount(address(buyToken), buyAmount);
        (
            ISignatureTransfer.PermitBatchTransferFrom memory permit,
            ExecutionWitness memory witness,
            bytes memory signature
        ) = prepareSellTokens(sellAmount, sellAmount, buyTokens);
        uint256 bidAmount = 1e3;
        (
            AdapterExecutionParams memory adapterExecutionParams,
            bytes memory adapterSignature
        ) = prepareTokensForOpportunityAdapter(
                ExecutionParams({permit: permit, witness: witness}),
                signature,
                bidAmount
            );
        bytes memory permission = abi.encode(address(admin), signature);
        MulticallData[] memory multicallData = new MulticallData[](1);
        multicallData[0] = MulticallData(
            bytes16("1"),
            address(adapterFactory),
            abi.encodeWithSelector(
                adapterFactory.executeOpportunity.selector,
                adapterExecutionParams,
                adapterSignature
            ),
            bidAmount
        );
        vm.prank(relayer);
        expressRelay.multicall(permission, multicallData);
        assertEq(sellToken.balanceOf(address(opportunityProvider)), 0);
        assertEq(buyToken.balanceOf(admin), buyAmount);
        assertEq(sellToken.balanceOf(makeAddr("buyer")), sellAmount);
    }

    function testExecuteWithoutBidAndDirectly(
        uint256 buyAmount,
        uint256 sellAmount
    ) public {
        ExecutionWitness memory buyWitness = prepareBuyTokens(
            admin,
            buyAmount,
            buyAmount,
            address(expressRelay)
        );
        (
            ISignatureTransfer.PermitBatchTransferFrom memory permit,
            ExecutionWitness memory witness,
            bytes memory signature
        ) = prepareSellTokens(sellAmount, sellAmount, buyWitness.buyTokens);
        ExecutionParams memory params = ExecutionParams({
            permit: permit,
            witness: witness
        });

        bytes memory permission = abi.encode(address(admin), signature);
        MulticallData[] memory multicallData = new MulticallData[](1);
        multicallData[0] = MulticallData(
            bytes16("1"),
            address(opportunityProvider),
            abi.encodeWithSelector(
                opportunityProvider.execute.selector,
                params,
                signature
            ),
            0
        );
        vm.prank(relayer);
        expressRelay.multicall(permission, multicallData);
        assertEq(sellToken.balanceOf(address(opportunityProvider)), 0);
        assertEq(buyToken.balanceOf(admin), buyAmount);
        assertEq(sellToken.balanceOf(address(expressRelay)), sellAmount);
    }

    function testRevertWhenCallExecuteDirectly(
        uint256 buyAmount,
        uint256 sellAmount
    ) public {
        ExecutionWitness memory buyWitness = prepareBuyTokens(
            admin,
            buyAmount,
            buyAmount,
            address(this)
        );
        (
            ISignatureTransfer.PermitBatchTransferFrom memory permit,
            ExecutionWitness memory witness,
            bytes memory signature
        ) = prepareSellTokens(sellAmount, sellAmount, buyWitness.buyTokens);
        ExecutionParams memory params = ExecutionParams({
            permit: permit,
            witness: witness
        });
        vm.expectRevert(InvalidOpportunity.selector);
        opportunityProvider.execute(params, signature);
    }
}

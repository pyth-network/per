# Opportunity Adapter Contract

This contract executes an `opportunity` on behalf of an `executor` in a secure manner.
An opportunity refers to any arbitrary contract call with pre-defined expectations of token exchanges.
The opportunity adapter handles routing arbitrary calldata to an external contract along with assertions around the
quantity of ETH and different tokens exchanged as a result of the contract call.
The main function `executeOpportunity` accepts an `ExecutionParams` type.

```solidity
struct TokenAmount {
  address token;
  uint256 amount;
}

struct ExecutionParams {
  ISignatureTransfer.PermitBatchTransferFrom permit;
  ExecutionWitness witness;
}

struct ExecutionWitness {
  TokenAmount[] buyTokens;
  bytes targetCalldata;
  uint256 targetCallValue;
  address targetContract;
  address executor;
  uint256 bidAmount;
}

```

The `ISignatureTransfer.PermitBatchTransferFrom` struct can be found [here](https://github.com/Uniswap/permit2/blob/cc56ad0f3439c502c246fc5cfcc3db92bb8b7219/src/interfaces/ISignatureTransfer.sol#L51-L58). This struct will contain the token(s) the user intends to sell and a nonce and deadline for the signature's validity.

by calling the `executeOpportunity` function the contract will:

1. Verify the parameters are valid:
   1. Verify it is being called from the `expressRelay` contract
   2. Verify that the executor is the owner of the contract
   3. Verify the `targetContract` is not itself or the permit2 contract
   4. Verifies there are no duplicate token addresses in `permit.permitted` or `witness.buyTokens`
2. Transfers the permitted "sell tokens" to itself via permit2 (which handles validation of the provided signature) and approves the `targetContract` to use them
3. Converts the necessary amount of Wrapped ETH received from the executor to ETH to be used as the value of the call
4. Calls the `targetContract` with `targetCalldata` and `targetCallValue`
5. Revokes the allowances of the `targetContract` over the permitted sell tokens
6. Checks that the contract has received the tokens specified in `buyTokens`
7. Similar to 3, transfers `bidAmount` Wrapped ETH from executor and sends it to the express relay contract as the bid.
8. Transfers the `buyTokens` back to the `executor`

If any of the mentioned steps fail the whole call will revert.

⚠️ Calling any ERC20 tokens with custom/malicious behaviour should be handled by the callee
and is out of the scope for this contract.

⚠️ In cases where less than the specified amount of tokens is used by the target call,
the remaining tokens will remain in the contract. Similarly, if less than the specified amount of (Wrapped) ETH is used by the target call, the remainder will remain in the contract as WETH. The owner of this contract can withdraw those assets via the `withdrawToken` function.

⚠️ If any ETH is transferred to the contract, the owner can withdraw the ETH via the `withdrawEth` function.

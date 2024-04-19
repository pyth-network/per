# Opportunity Adapter Contract

This contract executes an `opportunity` on behalf of an `executor` in a secure manner.
The main function `executeOpportunity` accepts an `ExecutionParams` type.
The `executor` should allow this contract to spend their tokens on their behalf using the ERC20 `approve` method.

```solidity
struct TokenAmount {
  address token;
  uint256 amount;
}

struct ExecutionParams {
  TokenAmount[] sellTokens;
  TokenAmount[] buyTokens;
  address executor;
  address targetContract;
  bytes targetCalldata;
  uint256 targetCallValue;
  uint256 validUntil;
  uint256 bidAmount;
  bytes signature;
}

```

by calling the executeOpportunity the contract will:

1. Verify the parameters are valid:
   1. Verify it is being called from the `expressRelay` contract
   2. Verify that the executor actually signed this payload
   3. Verify this signature has not been used before
   4. Verify the block.timestamp is less than or equal to the `validUntil` parameter.
   5. Verifies there are no duplicate token addresses in `sellTokens` or `buyTokens`
2. Transfers the sellTokens to the contract itself and approves the `targetContract` to use them
3. Transfers `targetCallValue` Wrapped ETH from executor and converts them to ETH to be used as the value of the call
4. Calls the `targetContract` with `targetCalldata` and `targetCallValue`
5. Checks that the contract has received the tokens specified in `buyTokens`
6. Transfers the `buyTokens` back to the `executor`
7. Similar to 3, transfers `bidAmount` Wrapped ETH from executor and sends it to the express relay contract as the bid.

If any of the mentioned steps fail the whole call will revert.

⚠️ Calling any ERC20 tokens with custom/malicious behaviour should be handled by the callee
and is out of the scope for this contract.

⚠️ In cases where less than the specified amount of tokens is used by the target call,
the remaining tokens will remain in the contract and still approved to be used by the `targetContract`

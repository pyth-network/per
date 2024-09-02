# Express Relay Contract

This contract executes the calls submitted by the `relayer` while ensuring that a certain amount of bid is paid
per call and splits the bid to protocol and relayer according to the predefined configs.
The bids submitted to the contract are the winners of an auction concluded by the relayer in the auction server.

```solidity
struct MulticallData {
    bytes16 bidId;
    address targetContract;
    bytes targetCalldata;
    uint256 bidAmount;
    uint256 gasLimit;
    bool revertOnFailure;
}

function multicall(
    bytes calldata permissionKey,
    MulticallData[] calldata multicallData
)

function isPermissioned(
    address protocolFeeReceiver,
    bytes calldata permissionId
)
```

When the relayer calls the `multicall` function it specifies a `permissionKey` and a list of `multicallData`.

The contract will call the `targetContract` using the specified `targetCalldata` with `value:0`
and expects the balance of the contract to be increased by `bidAmount` if the call was successful.
Otherwise it will revert the the call and undo all the changes made by that call.
Note the revert here is only for a specific call and not the whole transaction.
A single transaction can contain multiple `MulticallData` where some of them fail and revert.

`gasLimit` specifies the maximum amount of gas that will be forwarded for the external call. `revertOnFailure` is a boolean that determines whether to revert the entire `ExpressRelay` transaction if the external call fails. This is only intended to be set to `true` for simulation purposes; for actual production on-chain submissions, this is intended to be set to `false` so that no individual external call's failure causes failure of the entire `Express Relay` transaction.

`isPermissioned` will return true for the specified `permissionKey`
if and only if it is an inner call initiated by the `multicall` with the same `permissionKey`.

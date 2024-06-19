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
The revert here is only for a specific call and not the whole transaction.
A single transaction can contain multiple `MulticallData` where some of them fail and revert.

`isPermissioned` will return true for the specified `permissionKey`
if and only if it is an inner call initiated by the `multicall` with the same `permissionKey`.

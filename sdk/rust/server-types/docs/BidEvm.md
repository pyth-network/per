# BidEvm

## Properties

| Name                | Type                                        | Description                                            | Notes |
| ------------------- | ------------------------------------------- | ------------------------------------------------------ | ----- |
| **chain_id**        | **String**                                  | The chain id for bid.                                  |
| **id**              | **String**                                  | The unique id for bid.                                 |
| **initiation_time** | **String**                                  | The time server received the bid formatted in rfc3339. |
| **profile_id**      | **String**                                  | The profile id for the bid owner.                      |
| **bid_amount**      | **String**                                  | Amount of bid in wei.                                  |
| **gas_limit**       | **String**                                  | The gas limit for the contract call.                   |
| **permission_key**  | **String**                                  | The permission key for bid.                            |
| **status**          | [**models::BidStatusEvm**](BidStatusEvm.md) |                                                        |
| **target_calldata** | **String**                                  | Calldata for the contract call.                        |
| **target_contract** | **String**                                  | The contract address to call.                          |

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)

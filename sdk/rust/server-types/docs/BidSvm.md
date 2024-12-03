# BidSvm

## Properties

| Name                | Type                                        | Description                                                                                                              | Notes |
| ------------------- | ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | ----- |
| **chain_id**        | **String**                                  | The chain id for bid.                                                                                                    |
| **id**              | **String**                                  | The unique id for bid.                                                                                                   |
| **initiation_time** | **String**                                  | The time server received the bid formatted in rfc3339.                                                                   |
| **profile_id**      | **String**                                  | The profile id for the bid owner.                                                                                        |
| **bid_amount**      | **i64**                                     | Amount of bid in lamports.                                                                                               |
| **permission_key**  | **String**                                  | The permission key for bid in base64 format. This is the concatenation of the permission account and the router account. |
| **status**          | [**models::BidStatusSvm**](BidStatusSvm.md) |                                                                                                                          |
| **transaction**     | **String**                                  | The transaction of the bid.                                                                                              |

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)

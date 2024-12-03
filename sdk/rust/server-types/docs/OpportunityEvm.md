# OpportunityEvm

## Properties

| Name                  | Type                                                 | Description                                                              | Notes |
| --------------------- | ---------------------------------------------------- | ------------------------------------------------------------------------ | ----- |
| **buy_tokens**        | [**Vec<models::TokenAmountEvm>**](TokenAmountEvm.md) |                                                                          |
| **chain_id**          | **String**                                           | The chain id where the opportunity will be executed.                     |
| **permission_key**    | **String**                                           | The permission key required for successful execution of the opportunity. |
| **sell_tokens**       | [**Vec<models::TokenAmountEvm>**](TokenAmountEvm.md) |                                                                          |
| **target_call_value** | **String**                                           | The value to send with the contract call.                                |
| **target_calldata**   | **String**                                           | Calldata for the target contract call.                                   |
| **target_contract**   | **String**                                           | The contract address to call for execution of the opportunity.           |
| **version**           | **String**                                           |                                                                          |
| **creation_time**     | **i32**                                              | Creation time of the opportunity (in microseconds since the Unix epoch). |
| **opportunity_id**    | **String**                                           | The opportunity unique id.                                               |

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)

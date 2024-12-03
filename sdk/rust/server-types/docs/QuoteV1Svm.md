# QuoteV1Svm

## Properties

| Name                            | Type                                            | Description                                                                                            | Notes |
| ------------------------------- | ----------------------------------------------- | ------------------------------------------------------------------------------------------------------ | ----- |
| **chain_id**                    | **String**                                      | The chain id for the quote.                                                                            |
| **expiration_time**             | **i64**                                         | The expiration time of the quote (in seconds since the Unix epoch).                                    |
| **input_token**                 | [**models::TokenAmountSvm**](TokenAmountSvm.md) |                                                                                                        |
| **maximum_slippage_percentage** | **f64**                                         | The maximum slippage percentage that the user is willing to accept.                                    |
| **output_token**                | [**models::TokenAmountSvm**](TokenAmountSvm.md) |                                                                                                        |
| **transaction**                 | **String**                                      | The signed transaction for the quote to be executed on chain which is valid until the expiration time. |

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)

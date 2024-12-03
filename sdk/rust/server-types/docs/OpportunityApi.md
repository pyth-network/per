# \OpportunityApi

All URIs are relative to _http://localhost_

| Method                                                             | HTTP request                                     | Description                                                         |
| ------------------------------------------------------------------ | ------------------------------------------------ | ------------------------------------------------------------------- |
| [**delete_opportunities**](OpportunityApi.md#delete_opportunities) | **DELETE** /v1/opportunities                     | Delete all opportunities for specified data.                        |
| [**get_opportunities**](OpportunityApi.md#get_opportunities)       | **GET** /v1/opportunities                        | Fetch opportunities ready for execution or historical opportunities |
| [**opportunity_bid**](OpportunityApi.md#opportunity_bid)           | **POST** /v1/opportunities/{opportunity_id}/bids | Bid on opportunity.                                                 |
| [**post_opportunity**](OpportunityApi.md#post_opportunity)         | **POST** /v1/opportunities                       | Submit an opportunity ready to be executed.                         |
| [**post_quote**](OpportunityApi.md#post_quote)                     | **POST** /v1/opportunities/quote                 | Submit a quote request.                                             |

## delete_opportunities

> delete_opportunities(opportunity_delete)
> Delete all opportunities for specified data.

### Parameters

| Name                   | Type                                          | Description | Required   | Notes |
| ---------------------- | --------------------------------------------- | ----------- | ---------- | ----- |
| **opportunity_delete** | [**OpportunityDelete**](OpportunityDelete.md) |             | [required] |

### Return type

(empty response body)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

## get_opportunities

> Vec<models::Opportunity> get_opportunities(chain_id, mode, permission_key, from_time, limit)
> Fetch opportunities ready for execution or historical opportunities

depending on the mode. You need to provide `chain_id` for historical mode. Opportunities are sorted by creation time in ascending order. Total number of opportunities returned is capped by the server to preserve bandwidth.

### Parameters

| Name               | Type                                       | Description                                                                                                             | Required | Notes             |
| ------------------ | ------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------- | -------- | ----------------- |
| **chain_id**       | Option<**String**>                         |                                                                                                                         |          |
| **mode**           | Option<[**models::OpportunityMode**](.md)> | Get opportunities in live or historical mode.                                                                           |          | [default to live] |
| **permission_key** | Option<**String**>                         | The permission key to filter the opportunities by. Used only in historical mode.                                        |          |
| **from_time**      | Option<**String**>                         | The time to get the opportunities from.                                                                                 |          |
| **limit**          | Option<**i32**>                            | The maximum number of opportunities to return. Capped at 100; if more than 100 requested, at most 100 will be returned. |          |

### Return type

[**Vec<models::Opportunity>**](Opportunity.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

## opportunity_bid

> models::OpportunityBidResult opportunity_bid(opportunity_id, opportunity_bid_evm)
> Bid on opportunity.

### Parameters

| Name                    | Type                                          | Description              | Required   | Notes |
| ----------------------- | --------------------------------------------- | ------------------------ | ---------- | ----- |
| **opportunity_id**      | **String**                                    | Opportunity id to bid on | [required] |
| **opportunity_bid_evm** | [**OpportunityBidEvm**](OpportunityBidEvm.md) |                          | [required] |

### Return type

[**models::OpportunityBidResult**](OpportunityBidResult.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

## post_opportunity

> models::Opportunity post_opportunity(opportunity_create)
> Submit an opportunity ready to be executed.

The opportunity will be verified by the server. If the opportunity is valid, it will be stored in the database and will be available for bidding.

### Parameters

| Name                   | Type                                          | Description | Required   | Notes |
| ---------------------- | --------------------------------------------- | ----------- | ---------- | ----- |
| **opportunity_create** | [**OpportunityCreate**](OpportunityCreate.md) |             | [required] |

### Return type

[**models::Opportunity**](Opportunity.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

## post_quote

> models::Quote post_quote(quote_create)
> Submit a quote request.

The server will estimate the quote price, which will be used to create an opportunity. After a certain time, searcher bids are collected, the winning signed bid will be returned along with the estimated price.

### Parameters

| Name             | Type                              | Description | Required   | Notes |
| ---------------- | --------------------------------- | ----------- | ---------- | ----- |
| **quote_create** | [**QuoteCreate**](QuoteCreate.md) |             | [required] |

### Return type

[**models::Quote**](Quote.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

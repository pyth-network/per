# \BidApi

All URIs are relative to _http://localhost_

| Method                                                                   | HTTP request                         | Description                                                                   |
| ------------------------------------------------------------------------ | ------------------------------------ | ----------------------------------------------------------------------------- |
| [**get_bid_status**](BidApi.md#get_bid_status)                           | **GET** /v1/{chain_id}/bids/{bid_id} | Query the status of a specific bid.                                           |
| [**get_bid_status_deprecated**](BidApi.md#get_bid_status_deprecated)     | **GET** /v1/bids/{bid_id}            | Query the status of a specific bid.                                           |
| [**get_bids_by_time**](BidApi.md#get_bids_by_time)                       | **GET** /v1/{chain_id}/bids          | Returns at most 20 bids which were submitted after a specific time and chain. |
| [**get_bids_by_time_deprecated**](BidApi.md#get_bids_by_time_deprecated) | **GET** /v1/bids                     | Returns at most 20 bids which were submitted after a specific time.           |
| [**post_bid**](BidApi.md#post_bid)                                       | **POST** /v1/bids                    | Bid on a specific permission key for a specific chain.                        |

## get_bid_status

> models::BidStatus get_bid_status(chain_id, bid_id)
> Query the status of a specific bid.

### Parameters

| Name         | Type       | Description | Required   | Notes |
| ------------ | ---------- | ----------- | ---------- | ----- |
| **chain_id** | **String** |             | [required] |
| **bid_id**   | **String** |             | [required] |

### Return type

[**models::BidStatus**](BidStatus.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

## get_bid_status_deprecated

> models::BidStatus get_bid_status_deprecated(bid_id)
> Query the status of a specific bid.

This api is deprecated and will be removed soon. Use /v1/{chain_id}/bids/{bid_id} instead.

### Parameters

| Name       | Type       | Description         | Required   | Notes |
| ---------- | ---------- | ------------------- | ---------- | ----- |
| **bid_id** | **String** | Bid id to query for | [required] |

### Return type

[**models::BidStatus**](BidStatus.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

## get_bids_by_time

> models::Bids get_bids_by_time(chain_id, from_time)
> Returns at most 20 bids which were submitted after a specific time and chain.

If no time is provided, the server will return the first bids.

### Parameters

| Name          | Type               | Description               | Required   | Notes |
| ------------- | ------------------ | ------------------------- | ---------- | ----- |
| **chain_id**  | **String**         | The chain id to query for | [required] |
| **from_time** | Option<**String**> |                           |            |

### Return type

[**models::Bids**](Bids.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

## get_bids_by_time_deprecated

> models::Bids get_bids_by_time_deprecated(from_time)
> Returns at most 20 bids which were submitted after a specific time.

If no time is provided, the server will return the first bids. This api is deprecated and will be removed soon. Use /v1/{chain_id}/bids instead.

### Parameters

| Name          | Type               | Description | Required | Notes |
| ------------- | ------------------ | ----------- | -------- | ----- |
| **from_time** | Option<**String**> |             |          |

### Return type

[**models::Bids**](Bids.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

## post_bid

> models::BidResult post_bid(bid_create)
> Bid on a specific permission key for a specific chain.

Your bid will be verified by the server. Depending on the outcome of the auction, a transaction containing your bid will be sent to the blockchain expecting the bid amount to be paid in the transaction.

### Parameters

| Name           | Type                          | Description | Required   | Notes |
| -------------- | ----------------------------- | ----------- | ---------- | ----- |
| **bid_create** | [**BidCreate**](BidCreate.md) |             | [required] |

### Return type

[**models::BidResult**](BidResult.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

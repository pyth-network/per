LIQUIDATION_SERVER_ENDPOINT = "http://localhost:9000"
AUCTION_SERVER_ENDPOINT = "http://localhost:9000/bid"

LIQUIDATION_SERVER_ENDPOINT_SURFACE = (
    f"{LIQUIDATION_SERVER_ENDPOINT}/liquidation/submit_opportunity"
)
LIQUIDATION_SERVER_ENDPOINT_GETOPPS = (
    f"{LIQUIDATION_SERVER_ENDPOINT}/liquidation/fetch_opportunities"
)
LIQUIDATION_SERVER_ENDPOINT_BID = (
    f"{LIQUIDATION_SERVER_ENDPOINT}/liquidation/bid_opportunity"
)

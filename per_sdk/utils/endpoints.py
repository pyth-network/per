BEACON_SERVER_ENDPOINT = "http://localhost:9000"
AUCTION_SERVER_ENDPOINT = "http://localhost:9000/bid"

BEACON_SERVER_ENDPOINT_SURFACE = (
    f"{BEACON_SERVER_ENDPOINT}/liquidation/submit_opportunity"
)
BEACON_SERVER_ENDPOINT_GETOPPS = (
    f"{BEACON_SERVER_ENDPOINT}/liquidation/fetch_opportunities"
)
BEACON_SERVER_ENDPOINT_BID = f"{BEACON_SERVER_ENDPOINT}/liquidation/bid_opportunity"

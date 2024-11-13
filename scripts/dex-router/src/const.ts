// Time to wait in milliseconds before submitting a router bid on opportunity
// We introduce this wait time, since this router bid is a backup to searchers normally fulfilling
export const OPPORTUNITY_WAIT_TIME_MS = 800;

// Maximum price for priority fees (per compute unit in microlamports). If set to null, then there is no maximum on the price per compute unit.
export const MAX_COMPUTE_UNIT_PRICE = null;

// Threshold for RPC staleness in seconds
export const HEALTH_RPC_THRESHOLD = 300;

// Interval to run the RPC health check in seconds
export const HEALTH_RPC_INTERVAL = 10;

// Interval to run the Express Relay health check in seconds
export const HEALTH_EXPRESS_RELAY_INTERVAL = 10;

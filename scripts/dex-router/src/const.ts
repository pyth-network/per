// Time to wait in milliseconds before submitting a router bid on opportunity
// We introduce this wait time, since this router bid is a backup to searchers normally fulfilling
export const OPPORTUNITY_WAIT_TIME_MS = 800;

// Maximum price for priority fees (per compute unit in microlamports). If set to null, then there is no maximum on the price per compute unit.
export const MAX_COMPUTE_UNIT_PRICE = null;

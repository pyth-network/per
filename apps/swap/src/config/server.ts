import "server-only";

/* eslint-disable n/no-process-env */

export const SOLANA_RPC = process.env.SOLANA_RPC || "http://localhost:8899";
export const ENDPOINT_EXPRESS_RELAY =
  process.env.ENDPOINT_EXPRESS_RELAY || "http://localhost:9000";

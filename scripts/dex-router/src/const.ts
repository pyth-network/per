import { PublicKey } from "@solana/web3.js";

// Time to wait before submitting a router bid on opportunity
// We introduce this wait time, since this router bid is a backup to searchers normally fulfilling
export const OPPORTUNITY_WAIT_TIME = 800;

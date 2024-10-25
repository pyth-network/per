import { PublicKey } from "@solana/web3.js";

// Time to wait before submitting a router bid on opportunity
// We introduce this wait time, since this router bid is a backup to searchers normally fulfilling
export const OPPORTUNITY_WAIT_TIME = 800;

export const LOOKUP_TABLE_ADDRESS: Record<string, PublicKey> = {
  "development-solana": new PublicKey(
    "DmFnfPhTs4B4cqPyxLusbgBUhXPPP139TQNe71Yx9JvV"
    // "7VFFawdrwqGRJV3VaSoHYb8mrre5L7b1nQnQddMJCwRo"
  ),
};

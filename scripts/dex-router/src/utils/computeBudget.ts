import {
  ComputeBudgetInstruction,
  ComputeBudgetProgram,
  TransactionInstruction,
} from "@solana/web3.js";
import { MAX_COMPUTE_UNIT_PRICE } from "../const";

/**
 * Filters the provided Compute Budget instructions to only include the last SetComputeUnitLimit and SetComputeUnitPrice instructions. If the SetComputeUnitPrice instruction exceeds the MAX_COMPUTE_UNIT_PRICE, it will be replaced with a SetComputeUnitPrice instruction with the MAX_COMPUTE_UNIT_PRICE.
 * @param ixs The Compute Budget instructions to filter
 */
export function filterComputeBudgetIxs(
  ixs: TransactionInstruction[]
): TransactionInstruction[] {
  if (ixs.some((ix) => !ix.programId.equals(ComputeBudgetProgram.programId))) {
    throw new Error("All instructions must be for the Compute Budget program");
  }
  let ixsFiltered: TransactionInstruction[] = [];

  const typesComputeBudget = ixs.map((ix) =>
    ComputeBudgetInstruction.decodeInstructionType(ix)
  );

  // for now we filter out the SetComputeUnitLimit instruction because we don't care about the limit and want to ensure the tx doesn't fail bc of low limit
  // we only care about the last SetComputeUnitPrice because only the last will be enforced in transaction processing
  const lastIxSetCuPrice = typesComputeBudget.lastIndexOf(
    "SetComputeUnitPrice"
  );
  if (lastIxSetCuPrice !== -1) {
    const cuPrice = ComputeBudgetInstruction.decodeSetComputeUnitPrice(
      ixs[lastIxSetCuPrice]
    );
    if (
      MAX_COMPUTE_UNIT_PRICE !== null &&
      cuPrice.microLamports > MAX_COMPUTE_UNIT_PRICE
    ) {
      ixsFiltered.push(
        ComputeBudgetProgram.setComputeUnitPrice({
          microLamports: MAX_COMPUTE_UNIT_PRICE,
        })
      );
    } else {
      ixsFiltered.push(ixs[lastIxSetCuPrice]);
    }
  }

  return ixsFiltered;
}

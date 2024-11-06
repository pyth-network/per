import {
  ComputeBudgetInstruction,
  ComputeBudgetProgram,
  TransactionInstruction,
} from "@solana/web3.js";
import { MAX_COMPUTE_UNIT_PRICE } from "../const";

export function filterComputeBudgetIxs(
  ixs: TransactionInstruction[]
): TransactionInstruction[] {
  let ixsFiltered: TransactionInstruction[] = [];

  // we only care about the last SetComputeUnitLimit and SetComputeUnitPrice because only the last of each will be enforced in transaction processing
  const typesComputeBudget = ixs.map((ix) => {
    return ComputeBudgetInstruction.decodeInstructionType(ix);
  });

  const lastIxSetCuLimit = typesComputeBudget.lastIndexOf(
    "SetComputeUnitLimit"
  );
  if (lastIxSetCuLimit !== -1) {
    ixsFiltered.push(ixs[lastIxSetCuLimit]);
  }
  console.log(lastIxSetCuLimit);

  const lastIxSetCuPrice = typesComputeBudget.lastIndexOf(
    "SetComputeUnitPrice"
  );
  console.log(lastIxSetCuPrice);
  if (lastIxSetCuPrice !== -1) {
    const cuPrice = ComputeBudgetInstruction.decodeSetComputeUnitPrice(
      ixs[lastIxSetCuPrice]
    );
    console.log(`GETTING CU PRICE IX ${cuPrice.microLamports}`);
    if (
      MAX_COMPUTE_UNIT_PRICE !== null &&
      cuPrice.microLamports > MAX_COMPUTE_UNIT_PRICE
    ) {
      ixsFiltered.push(
        ComputeBudgetProgram.setComputeUnitPrice({
          microLamports: MAX_COMPUTE_UNIT_PRICE,
        })
      );
      console.log(`replacing with ${MAX_COMPUTE_UNIT_PRICE}:`);
      console.log(
        ComputeBudgetProgram.setComputeUnitPrice({
          microLamports: MAX_COMPUTE_UNIT_PRICE,
        })
      );
    } else {
      ixsFiltered.push(ixs[lastIxSetCuPrice]);
      console.log(`keeping:`);
      console.log(ixs[lastIxSetCuPrice]);
    }
  }

  return ixsFiltered;
}

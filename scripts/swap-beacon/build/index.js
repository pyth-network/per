"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.SwapBeacon = exports.SwapBeaconError = void 0;
exports.getSwapAdapterConfig = getSwapAdapterConfig;
const odos_1 = require("./adapter/odos");
const const_1 = require("./const");
const express_relay_evm_js_1 = require("@pythnetwork/express-relay-evm-js");
const viem_1 = require("viem");
const abi_1 = require("./abi");
class SwapBeaconError extends Error {}
exports.SwapBeaconError = SwapBeaconError;
function getSwapAdapterConfig(chainId) {
  const swapAdapterConfig = const_1.SWAP_ADAPTER_CONFIGS[chainId];
  if (!swapAdapterConfig) {
    throw new SwapBeaconError(
      `Opportunity adapter config not found for chain id: ${chainId}`
    );
  }
  return swapAdapterConfig;
}
class SwapBeacon {
  constructor(endpoint, _chainId) {
    this.endpoint = endpoint;
    this._chainId = _chainId;
    this.client = new express_relay_evm_js_1.Client(
      {
        baseUrl: endpoint,
      },
      undefined,
      this.opportunityHandler.bind(this)
    );
    this.chainId = _chainId;
    this.adapters = [new odos_1.OdosAdapter()];
  }
  async getOptimalAdapter(chainId, tokenIn, tokenOut, amountIn) {
    if (tokenIn === tokenOut) {
      return;
    }
    return this.adapters[0];
    // const prices = await Promise.all(
    //   this.adapters.map((adapter) =>
    //     adapter.getPrice(chainId, tokenIn, tokenOut, amountIn)
    //   )
    // );
    // return this.adapters[
    //   prices.reduce(
    //     (prev, curr, currIndex) => (prices[prev] < curr ? currIndex : prev),
    //     0
    //   )
    // ];
  }
  makeMulticallCalldata(
    opportunity,
    swapsSell,
    swapsBuy,
    sellTokens,
    buyTokens
  ) {
    const originalTargetCall = {
      targetContract: opportunity.targetContract,
      targetCalldata: opportunity.targetCalldata,
      targetCallValue: opportunity.targetCallValue,
      tokensToSend: opportunity.sellTokens.map((tokenAmount) => ({
        tokenAmount: tokenAmount,
        destination: opportunity.targetContract,
      })),
    };
    const swapsSellTargetCalls = swapsSell.map((swap) => ({
      targetContract: swap.targetContract,
      targetCalldata: swap.targetCalldata,
      targetCallValue: swap.targetCallValue,
      tokensToSend: swap.tokensToSend,
    }));
    const swapsBuyTargetCalls = swapsBuy.map((swap) => ({
      targetContract: swap.targetContract,
      targetCalldata: swap.targetCalldata,
      targetCallValue: swap.targetCallValue,
      tokensToSend: swap.tokensToSend,
    }));
    const multicallTargetCalls = [
      ...swapsSellTargetCalls,
      originalTargetCall,
      ...swapsBuyTargetCalls,
    ];
    return (0, viem_1.encodeFunctionData)({
      abi: [abi_1.multicallAbi],
      args: [[sellTokens, buyTokens, multicallTargetCalls]],
    });
  }
  extractTokenAmounts(extendedTargetCall) {
    let inputsAll = {};
    let outputsAll = {};
    for (let call of extendedTargetCall) {
      call.tokensToSend.forEach((tokenToSend) => {
        const token = tokenToSend.tokenAmount.token;
        let amount = tokenToSend.tokenAmount.amount;
        if (token in outputsAll) {
          const deduction = Math.min(Number(outputsAll[token]), Number(amount));
          outputsAll[token] -= BigInt(deduction);
          amount -= BigInt(deduction);
          if (outputsAll[token] === 0n) {
            delete outputsAll[token];
          }
        }
        if (amount > 0n) {
          inputsAll[token] = amount;
        }
      });
      call.tokensToReceive.forEach((tokenToReceive) => {
        const token = tokenToReceive.token;
        const amount = tokenToReceive.amount;
        if (token in outputsAll) {
          outputsAll[token] += amount;
        } else {
          outputsAll[token] = amount;
        }
      });
    }
    const inputsTokenAmount = Object.entries(inputsAll).map(
      ([token, amount]) => ({ token: token, amount: amount })
    );
    const outputsTokenAmount = Object.entries(outputsAll).map(
      ([token, amount]) => ({ token: token, amount: amount })
    );
    return [inputsTokenAmount, outputsTokenAmount];
  }
  createSwapOpportunity(opportunity, base, swapsSell, swapsBuy) {
    const targetContract =
      const_1.SWAP_ADAPTER_CONFIGS[opportunity.chainId].multicallAdapter;
    const targetCallValue =
      swapsSell.reduce((prev, curr) => prev + curr.targetCallValue, 0n) +
      swapsBuy.reduce((prev, curr) => prev + curr.targetCallValue, 0n) +
      opportunity.targetCallValue;
    const sellTokens = this.extractTokenAmounts(swapsSell)[0];
    const buyTokens = this.extractTokenAmounts(swapsBuy)[1];
    const targetCalldata = this.makeMulticallCalldata(
      opportunity,
      swapsSell,
      swapsBuy,
      sellTokens,
      buyTokens
    );
    console.log("ORIGINAL SELL TOKENS", opportunity.sellTokens);
    console.log("ORIGINAL BUY TOKENS", opportunity.buyTokens);
    console.log("=====================================");
    console.log("SELL TOKENS", sellTokens);
    console.log("BUY TOKENS", buyTokens);
    console.log("=====================================");
    console.log("SWAPS SELL SEND", swapsSell[0].tokensToSend);
    console.log("SWAPS SELL RECEIVE", swapsSell[0].tokensToReceive);
    console.log("SWAPS BUY SEND", swapsBuy[0].tokensToSend);
    console.log("SWAPS BUY RECEIVE", swapsBuy[0].tokensToReceive);
    return {
      ...opportunity,
      targetContract,
      targetCalldata,
      targetCallValue,
      sellTokens,
      buyTokens,
    };
  }
  async convertOpportunity(opportunity, base) {
    const promisesOptimalAdaptersSell = opportunity.sellTokens.map(
      (sellToken) =>
        this.getOptimalAdapter(
          opportunity.chainId,
          base,
          sellToken.token,
          1000000000n
        )
      // TODO: improve
    );
    const promisesOptimalAdaptersBuy = opportunity.buyTokens.map((buyToken) =>
      this.getOptimalAdapter(
        opportunity.chainId,
        buyToken.token,
        base,
        buyToken.amount
      )
    );
    const [optimalAdaptersSell, optimalAdaptersBuy] = await Promise.all([
      Promise.all(promisesOptimalAdaptersSell),
      Promise.all(promisesOptimalAdaptersBuy),
    ]);
    const swapsSell = (
      await Promise.all(
        optimalAdaptersSell.map(async (adapter, index) => {
          if (adapter === undefined) {
            return [];
          }
          return adapter.constructSwaps(
            opportunity.chainId,
            base,
            opportunity.sellTokens[index].token,
            undefined,
            opportunity.sellTokens[index].amount
          );
        })
      )
    ).reduce((acc, val) => acc.concat(val), []);
    const swapsBuy = (
      await Promise.all(
        optimalAdaptersBuy.map(async (adapter, index) => {
          if (adapter === undefined) {
            return [];
          }
          return adapter.constructSwaps(
            opportunity.chainId,
            opportunity.buyTokens[index].token,
            base,
            opportunity.buyTokens[index].amount,
            undefined
          );
        })
      )
    ).reduce((acc, val) => acc.concat(val), []);
    return this.createSwapOpportunity(opportunity, base, swapsSell, swapsBuy);
  }
  async opportunityHandler(opportunity) {
    console.log("GOT AN OPP");
    const swapAdapterConfig = getSwapAdapterConfig(opportunity.chainId);
    if (
      opportunity.targetContract.toLowerCase() ===
      swapAdapterConfig.multicallAdapter.toLowerCase()
    ) {
      return;
    }
    await Promise.all(
      swapAdapterConfig.liquidAssets.map(async (base) => {
        const { opportunityId, ...params } = await this.convertOpportunity(
          opportunity,
          base
        );
        console.log("BASE ASSET IS", base);
        try {
          const result = await this.client.submitOpportunity(params);
          console.log("SUCCEEDED", result);
        } catch (error) {
          console.error(
            `Failed to submit opportunity ${opportunityId}: ${error}`
          );
        }
        // await this.client.submitOpportunity(params);
      })
    );
  }
  async start() {
    try {
      await this.client.subscribeChains([this.chainId]);
      console.log(
        `Subscribed to chain ${this.chainId}. Waiting for opportunities...`
      );
    } catch (error) {
      console.error(error);
      this.client.websocket?.close();
    }
  }
}
exports.SwapBeacon = SwapBeacon;
// const argv = yargs(hideBin(process.argv))
//   .option("endpoint", {
//     description:
//       "Express relay endpoint. e.g: https://per-staging.dourolabs.app/",
//     type: "string",
//     demandOption: true,
//   })
//   .option("chain-id", {
//     description: "Chain id to listen and convert opportunities for.",
//     type: "string",
//     demandOption: true,
//   })
//   .help()
//   .alias("help", "h")
//   .parseSync();
const endpoint = "https://pyth-express-relay-mainnet.asymmetric.re/";
const chainId = "mode";
async function run() {
  const beacon = new SwapBeacon(
    endpoint,
    chainId
    // argv.endpoint,
    // argv.chainId
  );
  await beacon.start();
}
run();

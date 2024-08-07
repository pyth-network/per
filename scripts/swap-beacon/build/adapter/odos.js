"use strict";
var __importDefault =
  (this && this.__importDefault) ||
  function (mod) {
    return mod && mod.__esModule ? mod : { default: mod };
  };
Object.defineProperty(exports, "__esModule", { value: true });
exports.OdosAdapter = void 0;
const axios_1 = __importDefault(require("axios"));
const index_1 = require("../index");
class OdosAdapter {
  constructor() {
    this.chainIds = ["mode"];
  }
  async getPrice(chainId, tokenIn, tokenOut, amountIn, amountOut) {
    if (typeof amountIn === "undefined") {
      throw new Error("amountIn must be defined");
    }
    if (typeof amountOut !== "undefined") {
      throw new Error("amountOut must not be defined");
    }
    // TODO: implement conversion from amountOut to amountIn as in constructSwaps
    const swapAdapterConfig = (0, index_1.getSwapAdapterConfig)(chainId);
    const response = await axios_1.default.post(
      "https://api.odos.xyz/sor/quote/v2",
      {
        chainId: swapAdapterConfig.chainIdNum,
        inputTokens: [
          {
            amount: amountIn.toString(),
            tokenAddress: tokenIn,
          },
        ],
        outputTokens: [
          {
            proportion: 1,
            tokenAddress: tokenOut,
          },
        ],
        slippageLimitPercent: 0.5,
        userAddr: swapAdapterConfig.multicallAdapter,
      }
    );
    return response.data.outTokens[0] / response.data.inTokens[0];
  }
  async constructSwaps(chainId, tokenIn, tokenOut, amountIn, amountOut) {
    const swapAdapterConfig = (0, index_1.getSwapAdapterConfig)(chainId);
    let estimatedAmountIn;
    if (amountIn === undefined) {
      if (amountOut === undefined) {
        throw new Error("amountIn or amountOut must be defined");
      }
      const tokensSupported0 = {
        tokenMap: {
          "0x0000000000000000000000000000000000000000": {
            name: "Ethereum",
            symbol: "ETH",
            decimals: 18,
            assetId: "eth",
            assetType: "eth",
            protocolId: "native",
            isRebasing: false,
          },
          "0x4200000000000000000000000000000000000006": {
            name: "Wrapped Ether",
            symbol: "WETH",
            decimals: 18,
            assetId: "weth",
            assetType: "eth",
            protocolId: null,
            isRebasing: false,
          },
          "0xd988097fb8612cc24eeC14542bC03424c656005f": {
            name: "USD Coin",
            symbol: "USDC",
            decimals: 6,
            assetId: "usdc",
            assetType: "usd",
            protocolId: "circle",
            isRebasing: false,
          },
          "0xf0F161fDA2712DB8b566946122a5af183995e2eD": {
            name: "Tether USD",
            symbol: "USDT",
            decimals: 6,
            assetId: "usdt",
            assetType: "usd",
            protocolId: "tether",
            isRebasing: false,
          },
          "0xcDd475325D6F564d27247D1DddBb0DAc6fA0a5CF": {
            name: "Wrapped BTC",
            symbol: "WBTC",
            decimals: 8,
            assetId: "wbtc",
            assetType: "btc",
            protocolId: null,
            isRebasing: false,
          },
          "0xFDa619b6d20975be80A10332cD39b9a4b0FAa8BB": {
            name: "SwapMode",
            symbol: "SMD",
            decimals: 18,
            assetId: "smd",
            assetType: "smd",
            protocolId: "swapmode",
            isRebasing: false,
          },
          "0x2416092f143378750bb29b79eD961ab195CcEea5": {
            name: "Renzo Restaked ETH",
            symbol: "ezETH",
            decimals: 18,
            assetId: "ezeth",
            assetType: "eth",
            protocolId: "renzo",
            isRebasing: false,
          },
          "0x028227c4dd1e5419d11Bb6fa6e661920c519D4F5": {
            name: "Bridged Wrapped eETH",
            symbol: "weETH",
            decimals: 18,
            assetId: "weeth",
            assetType: "eth",
            protocolId: "ether-fi",
            isRebasing: false,
          },
          "0x04C0599Ae5A44757c0af6F9eC3b93da8976c150A": {
            name: "Wrapped eETH Mode",
            symbol: "weETH.mode",
            decimals: 18,
            assetId: "weeth",
            assetType: "eth",
            protocolId: "ether-fi",
            isRebasing: false,
          },
          "0x80137510979822322193FC997d400D5A6C747bf7": {
            name: "StakeStone Ether",
            symbol: "STONE",
            decimals: 18,
            assetId: "stone",
            assetType: "eth",
            protocolId: "stakestone",
            isRebasing: false,
          },
          "0xe7903B1F75C534Dd8159b313d92cDCfbC62cB3Cd": {
            name: "rsETHWrapper",
            symbol: "wrsETH",
            decimals: 18,
            assetId: "wrseth",
            assetType: "eth",
            protocolId: "kelp-dao",
            isRebasing: false,
          },
          "0x59889b7021243dB5B1e065385F918316cD90D46c": {
            name: "Merlin BTC",
            symbol: "M-BTC",
            decimals: 18,
            assetId: "mbtc",
            assetType: "btc",
            protocolId: "merlin",
            isRebasing: false,
          },
          "0x6863fb62Ed27A9DdF458105B507C15b5d741d62e": {
            name: "KIM Token",
            symbol: "KIM",
            decimals: 18,
            assetId: "kim",
            assetType: "kim",
            protocolId: "kim",
            isRebasing: false,
          },
          "0xDfc7C877a950e49D2610114102175A06C2e3167a": {
            name: "Mode",
            symbol: "MODE",
            decimals: 18,
            assetId: "mode",
            assetType: "mode",
            protocolId: "mode",
            isRebasing: false,
          },
          "0x18470019bF0E94611f15852F7e93cf5D65BC34CA": {
            name: "Ionic",
            symbol: "ION",
            decimals: 18,
            assetId: "ion",
            assetType: "ion",
            protocolId: "ionic-protocol",
            isRebasing: false,
          },
          "0x71ef7EDa2Be775E5A7aa8afD02C45F059833e9d2": {
            name: "Ionic Wrapped Ether",
            symbol: "ionWETH",
            decimals: 18,
            assetId: "ionweth",
            assetType: "eth",
            protocolId: "ionic-protocol",
            isRebasing: false,
          },
          "0x2BE717340023C9e14C1Bb12cb3ecBcfd3c3fB038": {
            name: "Ionic USD Coin",
            symbol: "ionUSDC",
            decimals: 6,
            assetId: "ionusdc",
            assetType: "usd",
            protocolId: "ionic-protocol",
            isRebasing: false,
          },
          "0x94812F2eEa03A49869f95e1b5868C6f3206ee3D3": {
            name: "Ionic Tether USD",
            symbol: "ionUSDT",
            decimals: 6,
            assetId: "ionusdt",
            assetType: "usd",
            protocolId: "ionic-protocol",
            isRebasing: false,
          },
          "0xd70254C3baD29504789714A7c69d60Ec1127375C": {
            name: "Ionic Wrapped Bitcoin",
            symbol: "ionWBTC",
            decimals: 8,
            assetId: "ionwbtc",
            assetType: "btc",
            protocolId: "ionic-protocol",
            isRebasing: false,
          },
          "0x59e710215d45F584f44c0FEe83DA6d43D762D857": {
            name: "Ionic Renzo Restaked ETH",
            symbol: "ionezETH",
            decimals: 18,
            assetId: "ionezeth",
            assetType: "eth",
            protocolId: "ionic-protocol",
            isRebasing: false,
          },
          "0x9a9072302B775FfBd3Db79a7766E75Cf82bcaC0A": {
            name: "Ionic Wrapped eETH",
            symbol: "ionweETH",
            decimals: 18,
            assetId: "ionweeth",
            assetType: "eth",
            protocolId: "ionic-protocol",
            isRebasing: false,
          },
          "0x959FA710CCBb22c7Ce1e59Da82A247e686629310": {
            name: "Ionic StakeStone Ether",
            symbol: "ionSTONE",
            decimals: 18,
            assetId: "ionstone",
            assetType: "eth",
            protocolId: "ionic-protocol",
            isRebasing: false,
          },
          "0xA0D844742B4abbbc43d8931a6Edb00C56325aA18": {
            name: "Ionic Wrapped eETH Mode",
            symbol: "ionweETH.mode",
            decimals: 18,
            assetId: "ionweethmode",
            assetType: "eth",
            protocolId: "ionic-protocol",
            isRebasing: false,
          },
          "0x49950319aBE7CE5c3A6C90698381b45989C99b46": {
            name: "Ionic rsETHWrapper",
            symbol: "ionwrsETH",
            decimals: 18,
            assetId: "ionwrseth",
            assetType: "eth",
            protocolId: "ionic-protocol",
            isRebasing: false,
          },
          "0x19F245782b1258cf3e11Eda25784A378cC18c108": {
            name: "Ionic Merlin BTC",
            symbol: "ionM-BTC",
            decimals: 18,
            assetId: "ionmbtc",
            assetType: "btc",
            protocolId: "ionic-protocol",
            isRebasing: false,
          },
          "0x0000206329b97DB379d5E1Bf586BbDB969C63274": {
            name: "USDA",
            symbol: "USDA",
            decimals: 18,
            assetId: "usda",
            assetType: "usd",
            protocolId: "angle-protocol",
            isRebasing: false,
          },
          "0x8b2EeA0999876AAB1E7955fe01A5D261b570452C": {
            name: "Wrapped BMX Mode Liquidity Token",
            symbol: "wMLT",
            decimals: 18,
            assetId: "wmlt",
            assetType: "wmlt",
            protocolId: "morphex",
            isRebasing: false,
          },
          "0x66eEd5FF1701E6ed8470DC391F05e27B1d0657eb": {
            name: "BMX",
            symbol: "BMX",
            decimals: 18,
            assetId: "bmx",
            assetType: "bmx",
            protocolId: "morphex",
            isRebasing: false,
          },
        },
      }["tokenMap"];
      // convert all keys to lower case
      const tokensSupported = Object.fromEntries(
        Object.entries(tokensSupported0).map(([k, v]) => [k.toLowerCase(), v])
      );
      if (!tokensSupported[tokenIn.toLowerCase()]) {
        throw new Error("Token In not supported");
      }
      if (!tokensSupported[tokenOut.toLowerCase()]) {
        throw new Error("Token Out not supported");
      }
      const decimalsIn = tokensSupported[tokenIn.toLowerCase()].decimals;
      const decimalsOut = tokensSupported[tokenOut.toLowerCase()].decimals;
      // get estimated amountIn
      const responsePriceIn = (
        await axios_1.default.get(
          `https://api.odos.xyz/pricing/token/${swapAdapterConfig.chainIdNum}/${tokenIn}`
        )
      ).data.price;
      const responsePriceOut = (
        await axios_1.default.get(
          `https://api.odos.xyz/pricing/token/${swapAdapterConfig.chainIdNum}/${tokenOut}`
        )
      ).data.price;
      estimatedAmountIn = BigInt(
        Math.ceil(
          ((((Number(amountOut) * responsePriceIn) / responsePriceOut) *
            10 ** decimalsIn) /
            10 ** decimalsOut) *
            1.005
        )
      );
    } else {
      estimatedAmountIn = amountIn;
    }
    const responseQuote = await axios_1.default.post(
      "https://api.odos.xyz/sor/quote/v2",
      {
        chainId: swapAdapterConfig.chainIdNum,
        inputTokens: [
          {
            amount: estimatedAmountIn.toString(),
            tokenAddress: tokenIn,
          },
        ],
        outputTokens: [
          {
            proportion: 1,
            tokenAddress: tokenOut,
          },
        ],
        slippageLimitPercent: 0.5,
        userAddr: swapAdapterConfig.multicallAdapter,
      }
    );
    if (typeof amountOut !== "undefined") {
      if (responseQuote.data.outTokens[0] < amountOut) {
        throw new Error("Not enough output tokens");
      }
    }
    const pathId = responseQuote.data.pathId;
    const responseTx = await axios_1.default.post(
      "https://api.odos.xyz/sor/assemble",
      {
        pathId: pathId,
        simulate: false,
        userAddr: (0, index_1.getSwapAdapterConfig)(chainId).multicallAdapter,
      }
    );
    const targetCalldata = responseTx.data.transaction.data;
    const targetContract = responseTx.data.transaction.to;
    const targetCallValue = BigInt(responseTx.data.transaction.value);
    const tokensToSend = responseTx.data.inputTokens.map((inputToken) => ({
      tokenAmount: {
        token: inputToken.tokenAddress,
        amount: inputToken.amount,
      },
      destination: responseTx.data.transaction.to,
    }));
    const tokensToReceive = responseTx.data.outputTokens.map((outputToken) => ({
      token: outputToken.tokenAddress,
      amount: amountOut
        ? outputToken.amount
        : Math.floor(Number(outputToken.amount) * 0.995).toString(),
    }));
    return [
      {
        targetContract,
        targetCalldata,
        targetCallValue,
        tokensToSend,
        tokensToReceive,
      },
    ];
  }
}
exports.OdosAdapter = OdosAdapter;

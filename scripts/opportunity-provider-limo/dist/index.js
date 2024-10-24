"use strict";
var __createBinding =
  (this && this.__createBinding) ||
  (Object.create
    ? function (o, m, k, k2) {
        if (k2 === undefined) k2 = k;
        var desc = Object.getOwnPropertyDescriptor(m, k);
        if (
          !desc ||
          ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)
        ) {
          desc = {
            enumerable: true,
            get: function () {
              return m[k];
            },
          };
        }
        Object.defineProperty(o, k2, desc);
      }
    : function (o, m, k, k2) {
        if (k2 === undefined) k2 = k;
        o[k2] = m[k];
      });
var __setModuleDefault =
  (this && this.__setModuleDefault) ||
  (Object.create
    ? function (o, v) {
        Object.defineProperty(o, "default", { enumerable: true, value: v });
      }
    : function (o, v) {
        o["default"] = v;
      });
var __importStar =
  (this && this.__importStar) ||
  function (mod) {
    if (mod && mod.__esModule) return mod;
    var result = {};
    if (mod != null)
      for (var k in mod)
        if (k !== "default" && Object.prototype.hasOwnProperty.call(mod, k))
          __createBinding(result, mod, k);
    __setModuleDefault(result, mod);
    return result;
  };
var __awaiter =
  (this && this.__awaiter) ||
  function (thisArg, _arguments, P, generator) {
    function adopt(value) {
      return value instanceof P
        ? value
        : new P(function (resolve) {
            resolve(value);
          });
    }
    return new (P || (P = Promise))(function (resolve, reject) {
      function fulfilled(value) {
        try {
          step(generator.next(value));
        } catch (e) {
          reject(e);
        }
      }
      function rejected(value) {
        try {
          step(generator["throw"](value));
        } catch (e) {
          reject(e);
        }
      }
      function step(result) {
        result.done
          ? resolve(result.value)
          : adopt(result.value).then(fulfilled, rejected);
      }
      step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
  };
var __importDefault =
  (this && this.__importDefault) ||
  function (mod) {
    return mod && mod.__esModule ? mod : { default: mod };
  };
Object.defineProperty(exports, "__esModule", { value: true });
const yargs_1 = __importDefault(require("yargs"));
const helpers_1 = require("yargs/helpers");
const fs = __importStar(require("fs"));
const web3_js_1 = require("@solana/web3.js");
const anchor = __importStar(require("@coral-xyz/anchor"));
const limo = __importStar(require("@kamino-finance/limo-sdk"));
const decimal_js_1 = require("decimal.js");
const utils_1 = require("@kamino-finance/limo-sdk/dist/utils");
function readFile(path) {
  const data = fs.readFileSync(path, "utf8");
  return JSON.parse(data);
}
function loadOpportunities(path) {
  const opportunityPairs = readFile(path);
  return opportunityPairs.map((opportunity) => {
    return {
      token1: {
        mint: new web3_js_1.PublicKey(opportunity.token1.mint),
        symbol: opportunity.token1.symbol,
      },
      token2: {
        mint: new web3_js_1.PublicKey(opportunity.token2.mint),
        symbol: opportunity.token2.symbol,
      },
      randomizeSides: opportunity.randomizeSides,
      minAmountNotional: opportunity.minAmountNotional,
      maxAmountNotional: opportunity.maxAmountNotional,
    };
  });
}
const decimals = {};
const prices = {};
function getDecimals(connection, token) {
  return __awaiter(this, void 0, void 0, function* () {
    const index = token.mint.toBase58();
    if (decimals[index] === undefined) {
      decimals[index] = yield (0, utils_1.getMintDecimals)(
        connection,
        token.mint
      );
    }
    return decimals[index];
  });
}
function getPrice(connection, token) {
  return __awaiter(this, void 0, void 0, function* () {
    const index = token.symbol;
    if (prices[index] === undefined) {
      const url = `https://api.binance.com/api/v3/ticker/price?symbol=${token.symbol}USDT`;
      const response = yield fetch(url);
      const data = yield response.json();
      const price = parseFloat(data.price);
      if (isNaN(price)) {
        throw new Error(`Invalid price: ${data.price}`);
      }
      prices[index] = price;
    }
    const mintDecimals = yield getDecimals(connection, token);
    return prices[index] / Math.pow(10, mintDecimals);
  });
}
function createOpportunities(
  skExecutor,
  limoClient,
  opportunitiesPath,
  count,
  edge
) {
  return __awaiter(this, void 0, void 0, function* () {
    const opportunities = loadOpportunities(opportunitiesPath);
    for (let i = 0; i < opportunities.length; i++) {
      const opportunity = opportunities[i];
      for (let j = 0; j < count; j++) {
        let inputToken = opportunity.token1;
        let outputToken = opportunity.token2;
        if (opportunity.randomizeSides) {
          if (Math.random() > 0.5) {
            inputToken = opportunity.token2;
            outputToken = opportunity.token1;
          }
        }
        const priceInput = yield getPrice(
          limoClient.getConnection(),
          inputToken
        );
        const priceOutput = yield getPrice(
          limoClient.getConnection(),
          outputToken
        );
        const notional =
          Math.random() *
            (opportunity.maxAmountNotional - opportunity.minAmountNotional) +
          opportunity.minAmountNotional;
        const amountInput = (notional * (1 + edge / 10000)) / priceInput;
        const amountOutput = notional / priceOutput;
        console.log("Creating opportunity:");
        const decimalsInput = yield getDecimals(
          limoClient.getConnection(),
          inputToken
        );
        const decimalsOutput = yield getDecimals(
          limoClient.getConnection(),
          outputToken
        );
        console.log(
          `Input: ${inputToken.symbol}, ${
            amountInput / Math.pow(10, decimalsInput)
          }`
        );
        console.log(
          `Output: ${outputToken.symbol}, ${
            amountOutput / Math.pow(10, decimalsOutput)
          }`
        );
        // TODO: TEST THIS BEFORE UNCOMMENTING
        //   const signature = "testing";
        const signature = yield limoClient.createOrderGeneric(
          skExecutor,
          inputToken.mint,
          outputToken.mint,
          new decimal_js_1.Decimal(amountInput),
          new decimal_js_1.Decimal(amountOutput)
        );
        console.log(`Created opportunity: ${signature}`);
      }
    }
  });
}
const argv = (0, yargs_1.default)((0, helpers_1.hideBin)(process.argv))
  .option("sk-payer", {
    description:
      "Secret key of address to submit transactions with. If action is 'create', this keypair creates the order in Limo. In 64-byte base58 format",
    type: "string",
    demandOption: true,
  })
  .option("global-config", {
    description: "Global config address",
    type: "string",
    demandOption: true,
  })
  .option("endpoint-svm", {
    description: "SVM RPC endpoint",
    type: "string",
    demandOption: true,
  })
  .option("opportunities", {
    description: "Path to opportunities file",
    type: "string",
    default: "opportunities.json",
  })
  .option("count", {
    description: "Number of opportunities to create",
    type: "number",
    default: 10,
  })
  .option("edge", {
    description:
      "Markup of the sold-off assets relative to the purchased assets, in basis points. e.g.: 100 = 1%",
    type: "number",
    default: 100,
  })
  .help()
  .alias("help", "h")
  .parseSync();
function run() {
  return __awaiter(this, void 0, void 0, function* () {
    const skExecutor = web3_js_1.Keypair.fromSecretKey(
      anchor.utils.bytes.bs58.decode(argv["sk-payer"])
    );
    console.log(`Using payer/creator: ${skExecutor.publicKey.toBase58()}`);
    const globalConfig = new web3_js_1.PublicKey(argv.globalConfig);
    console.log(`Using global config: ${globalConfig.toBase58()}`);
    const limoClient = new limo.LimoClient(
      new web3_js_1.Connection(argv.endpointSvm),
      globalConfig
    );
    yield createOpportunities(
      skExecutor,
      limoClient,
      argv.opportunities,
      argv.count,
      argv.edge
    );
  });
}
run();

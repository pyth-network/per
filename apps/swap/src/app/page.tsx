"use client";

import { Button } from "@/components/ui/button";
import { useConnection, useWallet } from "@solana/wallet-adapter-react";
import "@solana/wallet-adapter-react-ui/styles.css";
import {
  WalletDisconnectButton,
  WalletMultiButton,
} from "@/components/WalletButton";
import { PublicKey } from "@solana/web3.js";
import { useCallback, useState } from "react";
import { useExpressRelayClient } from "@/components/ExpressRelayProvider";
import bs58 from "bs58";

const USDC = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
const USDT = new PublicKey("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB");

export default function Home() {
  const { publicKey, signTransaction } = useWallet();
  const { connection } = useConnection();
  const expressRelayClient = useExpressRelayClient();

  const [log, setLog] = useState<string[]>([]);

  const handleClick = useCallback(() => {
    const inner = async () => {
      if (!publicKey || !signTransaction) return;
      setLog(["Getting quote..."]);
      // random to avoid same opportunity submitted recently error
      const amount = 1000000 + Math.floor(Math.random() * 1000);
      setLog((log) => [...log, `Selling ${amount / 1e6} USDT for USDC`]);
      const quote = await expressRelayClient.getQuote({
        chainId: "development-solana",
        inputTokenMint: USDC,
        outputTokenMint: USDT,
        referralFeeInfo: {
          router: publicKey,
          referralFeeBps: 0,
        },
        userWallet: publicKey,
        specifiedTokenAmount: {
          amount,
          side: "input",
        },
      });

      setLog((log) => [
        ...log,
        JSON.stringify(
          {
            inputAmount: quote.inputToken.amount.toString(),
            outputAmount: quote.outputToken.amount.toString(),
            expirationTime: quote.expirationTime.toISOString(),
          },
          null,
          2,
        ),
      ]);
      const signedTransaction = await signTransaction(quote.transaction);
      const accountPosition = signedTransaction.message
        .getAccountKeys()
        .staticAccountKeys.findIndex((key) => key.equals(publicKey));
      const signature = signedTransaction.signatures[accountPosition];
      if (!signature) {
        throw new Error("Signature not found");
      }
      const tx = await expressRelayClient.submitQuote({
        chainId: "development-solana",
        referenceId: quote.referenceId,
        userSignature: bs58.encode(signature),
      });
      const tx_hash = tx.signatures[0];
      if (!tx_hash) {
        throw new Error("Transaction hash not found");
      }
      setLog((log) => [...log, "Submitted quote: " + bs58.encode(tx_hash)]);
    };
    inner().catch((error) => {
      setLog((log) => [...log, error.message]);
      console.error(error);
    });
  }, [expressRelayClient, publicKey, signTransaction, connection]);

  const canSwap = publicKey && signTransaction;
  return (
    <main>
      <div className="m-auto w-2/4 parent space-y-2">
        <h1>Express Relay Swap testing UI</h1>
        <WalletMultiButton />
        <WalletDisconnectButton />
        <p>Public Key: </p>
        <pre>{publicKey?.toBase58()}</pre>
        {canSwap && (
          <Button onClick={handleClick}>
            Click me to buy 1 USDC with USDT
          </Button>
        )}
        <pre>
          {log.map((line: string, i: number) => (
            <div key={i}>{line}</div>
          ))}
        </pre>
      </div>
    </main>
  );
}

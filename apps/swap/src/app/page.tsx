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
      try {
        // random to avoid same opportunity submitted recently error
        const amount = 1000000 + Math.floor(Math.random() * 1000);
        setLog((log) => [...log, `Selling ${amount / 1e6} USDT for USDC`]);
        const quote = await expressRelayClient.getQuote({
          chainId: "development-solana",
          inputTokenMint: USDC,
          outputTokenMint: USDT,
          router: publicKey,
          userWallet: publicKey,
          specifiedTokenAmount: {
            amount: amount,
            side: "input",
          },
        });
        setLog((log) => [...log, JSON.stringify(quote, null, 2)]);
        const signedTransaction = await signTransaction(quote.transaction);
        connection.sendTransaction(signedTransaction);
      } catch (error) {
        setLog((log) => [...log, error.message]);
        return;
      }
    };
    inner().catch((error) => {
      console.error(error);
    });
  }, [expressRelayClient, publicKey, signTransaction, connection]);

  const canSwap = publicKey && signTransaction;
  return (
    <main>
      <div className="m-auto w-2/4">
        <h1>Express Relay Swap testing UI</h1>
        <div className="my-3">
          <WalletMultiButton />
        </div>
        <WalletDisconnectButton />
        <p>Public Key: </p>
        <pre>{publicKey?.toBase58()}</pre>
        {canSwap && (
          <Button onClick={handleClick}>
            Click me to sell 1 USDT for USDC
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

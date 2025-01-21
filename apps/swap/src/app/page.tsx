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
  const { publicKey, sendTransaction } = useWallet();
  const { connection } = useConnection();
  const expressRelayClient = useExpressRelayClient();

  const [log, setLog] = useState<string[]>([]);

  const handleClick = useCallback(() => {
    const inner = async () => {
      if (!publicKey || !sendTransaction) return;
      setLog(["Getting quote..."]);
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
      const txHash = await sendTransaction(quote.transaction, connection);
      setLog((log) => [...log, `Transaction sent: ${txHash}`]);
    };
    inner().catch((error) => {
      setLog((log) => [...log, error.message]);
      console.error(error);
    });
  }, [expressRelayClient, publicKey, sendTransaction, connection]);

  const canSwap = publicKey && sendTransaction;
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

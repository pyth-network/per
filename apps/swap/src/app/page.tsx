"use client";

import { Button } from "@/components/ui/button";
import { useConnection, useWallet } from "@solana/wallet-adapter-react";
import "@solana/wallet-adapter-react-ui/styles.css";
import {
  WalletDisconnectButton,
  WalletMultiButton,
} from "@/components/WalletButton";
import { PublicKey } from "@solana/web3.js";
import { useCallback } from "react";
import { useExpressRelayClient } from "@/components/ExpressRelayProvider";

const USDC = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
const USDT = new PublicKey("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB");

export default function Home() {
  const { publicKey, signTransaction } = useWallet();
  const { connection } = useConnection();
  const expressRelayClient = useExpressRelayClient();

  const handleClick = useCallback(() => {
    const inner = async () => {
      if (!publicKey || !signTransaction) return;
      console.log("Getting quote...");
      const quote = await expressRelayClient.getQuote({
        chainId: "development-solana",
        inputTokenMint: USDC,
        outputTokenMint: USDT,
        router: publicKey,
        userWallet: publicKey,
        specifiedTokenAmount: {
          amount: Math.floor(Math.random() * 100000), // random to avoid same opportunity submitted recently error
          side: "input",
        },
      });
      const signedTransaction = await signTransaction(quote.transaction);
      connection.sendTransaction(signedTransaction);
    };
    inner().catch((error) => {
      console.error(error);
    });
  }, [expressRelayClient, publicKey, signTransaction, connection]);

  const canSwap = publicKey && signTransaction;
  return (
    <main>
      <WalletMultiButton />
      <WalletDisconnectButton />
      <p>Public Key: {publicKey?.toBase58()}</p>
      {canSwap && (
        <Button onClick={handleClick}>Click me to sell 1 USDT for USDC</Button>
      )}
    </main>
  );
}

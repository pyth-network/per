"use client"

import { Button } from "@/components/ui/button";
import { useConnection, useWallet } from "@solana/wallet-adapter-react";
import '@solana/wallet-adapter-react-ui/styles.css';
import { WalletDisconnectButton, WalletMultiButton } from "@/components/WalletButton";
import { PublicKey } from "@solana/web3.js";
import { useCallback } from "react";
import { useExpressRelayClient } from "@/components/ExpressRelayProvider";

const USDC = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
const USDT = new PublicKey("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB")

export default function Home() {
  const { publicKey, signTransaction } = useWallet();
  const {connection} = useConnection();
  const expressRelayClient = useExpressRelayClient()

  const handleClick = useCallback(() => {
    if (!publicKey || !signTransaction) return;
    console.log("Getting quote...");
    expressRelayClient.getQuote({
      chainId: "development-solana",
      inputTokenMint: USDC,
      outputTokenMint: USDT,
      router: publicKey,
      userWallet: publicKey,
      specifiedTokenAmount: {
        amount: 1000000,
        side: "input",
      },
    }).then(quote => {
      signTransaction(quote.transaction).then(signedTransaction => {
        connection.sendTransaction(signedTransaction);
      });
    }).catch(error => {
      console.error(error);
    });
  }, [expressRelayClient, publicKey, signTransaction, connection]);


  return (
    <main>
      <WalletMultiButton />
      <WalletDisconnectButton />
      <p>Public Key: {publicKey?.toBase58()}</p>
      {publicKey && <Button onClick={handleClick}>Click me to sell 1 USDT for USDC</Button>}
    </main>
  );
}

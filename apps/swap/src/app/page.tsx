"use client"

import { Button } from "@/components/ui/button";
import { useWallet } from "@solana/wallet-adapter-react";
import '@solana/wallet-adapter-react-ui/styles.css';
import { WalletDisconnectButton, WalletMultiButton } from "@/components/WalletButton";
import { Client } from "@pythnetwork/express-relay-js";
import { PublicKey } from "@solana/web3.js";

const USDC = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
const USDT = new PublicKey("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB")

export default function Home() {
  const { publicKey } = useWallet();
  const expressRelayClient = new Client({baseUrl: "https://per-staging.dourolabs.app/"})
  return (
    <main>
      <WalletMultiButton />
      <WalletDisconnectButton />
      <p>Public Key: {publicKey?.toBase58()}</p>
      {publicKey && <Button onClick={() => handleClick(expressRelayClient, publicKey)}>Click me to sell 1 USDC for USDT</Button>}
    </main>
  );
}

async function handleClick(expressRelayClient: Client, publicKey: PublicKey) {
  console.log("Getting quote...");
  const quote = await expressRelayClient.getQuote({
    chainId: "development-solana",
    inputTokenMint: USDC,
    outputTokenMint: USDT,
    router: publicKey,
    userWallet: publicKey,
    specifiedTokenAmount: {
      amount: 1000000,
      side: "input",
    },
  });
  console.log(quote);
}
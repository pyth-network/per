"use client"

import { Button } from "@/components/ui/button";
import { useWallet } from "@solana/wallet-adapter-react";
import '@solana/wallet-adapter-react-ui/styles.css';
import { WalletDisconnectButton, WalletMultiButton } from "@/components/WalletButton";

export default function Home() {
  const { publicKey } = useWallet();
  return (
    <main>
      <WalletMultiButton />
      <WalletDisconnectButton />
      <p>Public Key: {publicKey?.toBase58()}</p>
      {publicKey && <Button>Click me to sell 1 USD for SOL</Button>}
    </main>
  );
}

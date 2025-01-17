"use client"

import { useWallet } from "@solana/wallet-adapter-react";
import { WalletDisconnectButton, WalletMultiButton } from "@solana/wallet-adapter-react-ui";

export default function Home() {
    const { publicKey } = useWallet();
  return (
    <main>
      <WalletMultiButton />
      <WalletDisconnectButton />
      <h2>Welcome to Swap</h2>
      <p>Public Key: {publicKey?.toBase58()}</p>
    </main>
  );
}

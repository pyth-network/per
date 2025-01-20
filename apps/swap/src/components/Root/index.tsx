import type { ReactNode } from "react";
import { WalletProvider } from "../WalletProvider";
import { ExpressRelayProvider } from "../ExpressRelayProvider";
import{ SOLANA_RPC, ENDPOINT_EXPRESS_RELAY } from "@/config/server";

type Props = {
  children: ReactNode;
};

export const Root = ({ children }: Props) => {
  return (
    <WalletProvider endpoint={SOLANA_RPC}>
      <ExpressRelayProvider endpoint={ENDPOINT_EXPRESS_RELAY}>
        <html>
          <body>
            <div>
            <h1>Express Relay Swap!</h1>
            {children}
            </div>
        </body>
      </html>
    </ExpressRelayProvider>
    </WalletProvider>
  );
};

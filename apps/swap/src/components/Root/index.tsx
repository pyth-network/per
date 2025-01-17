import type { ReactNode } from "react";
import { WalletProvider } from "../WalletProvider";

type Props = {
  children: ReactNode;
};

export const Root = ({ children }: Props) => {
  return (
    <WalletProvider endpoint="https://api.mainnet-beta.solana.com">
      <html>
        <body>
          <div>
          <h1>Hello World!</h1>
          {children}
          </div>
        </body>
      </html>
    </WalletProvider>
  );
};

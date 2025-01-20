"use client";

import { Client } from "@pythnetwork/express-relay-js";
import { createContext, type ReactNode, useContext, useMemo } from "react";

type Props = {
  children?: ReactNode | ReactNode[] | undefined;
  endpoint: string;
};

const ExpressRelayClientContext = createContext<Client | undefined>(undefined);

export const useExpressRelayClientContext = (endpoint: string) => {
  const expressRelayClient = useMemo(
    () => new Client({ baseUrl: endpoint }),
    [endpoint]
  );
  return expressRelayClient;
};

export const useExpressRelayClient = () => {
  const expressRelayClient = useContext(ExpressRelayClientContext);
  if (expressRelayClient === undefined) {
    throw new Error(
      "This component must be wrapped in an ExpressRelayProvider"
    );
  }
  return expressRelayClient;
};

export const ExpressRelayProvider = ({ endpoint, ...props }: Props) => {
  const state = useExpressRelayClientContext(endpoint);
  return (
    <ExpressRelayClientContext.Provider value={state}>
      {props.children}
    </ExpressRelayClientContext.Provider>
  );
};

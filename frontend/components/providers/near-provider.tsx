"use client";

import "@near-wallet-selector/modal-ui/styles.css";
import { setupMyNearWallet } from "@near-wallet-selector/my-near-wallet";
import { setupMeteorWallet } from "@near-wallet-selector/meteor-wallet";
import { setupLedger } from "@near-wallet-selector/ledger";
import { setupNightly } from "@near-wallet-selector/nightly";
import { WalletSelectorProvider } from "@near-wallet-selector/react-hook";
import { getNearConfig } from "@/lib/near/config";
import type { ReactNode } from "react";

interface NearProviderProps {
  children: ReactNode;
}

export function NearProvider({ children }: NearProviderProps) {
  const config = getNearConfig();
  
  console.log('[NearProvider] Initializing with config:', {
    networkId: config.networkId,
    nodeUrl: config.nodeUrl,
    contractId: config.contractId
  });
  
  const walletSelectorConfig = {
    network: {
      networkId: config.networkId,
      nodeUrl: config.nodeUrl,
      helperUrl: config.helperUrl,
      explorerUrl: config.explorerUrl,
      indexerUrl: config.helperUrl, // Use helperUrl as indexerUrl fallback
    },
    // Removed createAccessKeyFor to avoid 0.25 NEAR charge on every login
    // Users will approve transactions in their wallet instead
    modules: [
      setupMyNearWallet(),
      setupMeteorWallet(),
      setupLedger(),
      setupNightly(),
    ],
  };

  return (
    <WalletSelectorProvider config={walletSelectorConfig}>
      {children}
    </WalletSelectorProvider>
  );
}

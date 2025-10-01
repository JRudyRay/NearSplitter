'use client';

import { createContext, useContext, useEffect, useState } from "react";
import { setupWalletSelector } from "@near-wallet-selector/core";
import { setupModal } from "@near-wallet-selector/modal-ui";
import { setupMeteorWallet } from "@near-wallet-selector/meteor-wallet";
import { setupLedger } from "@near-wallet-selector/ledger";
import { setupNightly } from "@near-wallet-selector/nightly";
import "@near-wallet-selector/modal-ui/styles.css";

interface SimpleNearContextValue {
  contractId: string;
  explorerUrl: string;
  signIn: () => Promise<void>;
  signOut: () => Promise<void>;
  accountId: string | null;
  status: "idle" | "loading" | "ready" | "error";
  
  // Contract interaction methods
  call: (methodName: string, args: Record<string, unknown>, options?: { attachedDeposit?: string; gas?: string }) => Promise<any>;
  view: <T>(methodName: string, args: Record<string, unknown>) => Promise<T>;
}

const SimpleNearContext = createContext<SimpleNearContextValue | undefined>(undefined);

export function SimpleNearProvider({ children }: { children: React.ReactNode }) {
  const [accountId, setAccountId] = useState<string | null>(null);
  const [walletSelector, setWalletSelector] = useState<any>(null);
  const [modal, setModal] = useState<any>(null);

  useEffect(() => {
    const initWallet = async () => {
      const selector = await setupWalletSelector({
        network: "testnet",
        modules: [
          setupMeteorWallet(),
          setupLedger(),
          setupNightly(),
        ],
      });

      const walletModal = setupModal(selector, {
        contractId: "hello.near-examples.testnet",
      });

      setWalletSelector(selector);
      setModal(walletModal);

      // Check if already signed in
      const state = selector.store.getState();
      if (state.accounts.length > 0) {
        setAccountId(state.accounts[0].accountId);
      }

      // Subscribe to account changes
      selector.store.observable.subscribe((state: any) => {
        if (state.accounts.length > 0) {
          setAccountId(state.accounts[0].accountId);
        } else {
          setAccountId(null);
        }
      });
    };

    initWallet().catch(console.error);
  }, []);

  const value: SimpleNearContextValue = {
    contractId: "hello.near-examples.testnet",
    explorerUrl: "https://testnet.nearblocks.io",
    signIn: async () => {
      if (modal) {
        modal.show();
      } else {
        console.log("Wallet selector not ready yet");
      }
    },
    signOut: async () => {
      if (walletSelector) {
        const wallet = await walletSelector.wallet();
        await wallet.signOut();
        setAccountId(null);
      }
    },
    accountId,
    status: walletSelector ? "ready" : "loading",
    
    // Contract interaction methods - still mocked for now
    call: async (methodName, args, options) => {
      console.log("Mock contract call:", { methodName, args, options });
      return { success: true, message: `Mock response for ${methodName}` };
    },
    view: async (methodName, args) => {
      console.log("Mock contract view:", { methodName, args });
      return { data: `Mock view response for ${methodName}` } as any;
    }
  };

  return <SimpleNearContext.Provider value={value}>{children}</SimpleNearContext.Provider>;
}

export function useSimpleNear(): SimpleNearContextValue {
  const context = useContext(SimpleNearContext);
  if (!context) {
    throw new Error("useSimpleNear must be used within a SimpleNearProvider");
  }
  return context;
}
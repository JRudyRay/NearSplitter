'use client';

import { createContext, useContext, useEffect, useState } from "react";
import { setupWalletSelector, type WalletSelector } from "@near-wallet-selector/core";
import { setupModal, type WalletSelectorModal } from "@near-wallet-selector/modal-ui";
import { setupMyNearWallet } from "@near-wallet-selector/my-near-wallet";
import { setupMeteorWallet } from "@near-wallet-selector/meteor-wallet";
import { setupLedger } from "@near-wallet-selector/ledger";
import { setupNightly } from "@near-wallet-selector/nightly";
import { getNearConfig } from "@/lib/near/config";
import "@near-wallet-selector/modal-ui/styles.css";

interface SimpleNearContextValue {
  contractId: string;
  explorerUrl: string;
  signIn: () => Promise<void>;
  signOut: () => Promise<void>;
  accountId: string | null;
  status: "idle" | "loading" | "ready" | "error";
  
  // Contract interaction methods
  call: (methodName: string, args: Record<string, unknown>, options?: { attachedDeposit?: string; gas?: string }) => Promise<unknown>;
  view: <T>(methodName: string, args: Record<string, unknown>) => Promise<T>;
}

const SimpleNearContext = createContext<SimpleNearContextValue | undefined>(undefined);

export function SimpleNearProvider({ children }: { children: React.ReactNode }) {
  const [accountId, setAccountId] = useState<string | null>(null);
  const [walletSelector, setWalletSelector] = useState<WalletSelector | null>(null);
  const [modal, setModal] = useState<WalletSelectorModal | null>(null);
  const config = getNearConfig();

  useEffect(() => {
    const initWallet = async () => {
      const selector = await setupWalletSelector({
        network: config.networkId as "testnet" | "mainnet",
        modules: [
          setupMyNearWallet(),
          setupMeteorWallet(),
          setupLedger(),
          setupNightly(),
        ],
      });

      const walletModal = setupModal(selector, {
        contractId: config.contractId,
      });

      setWalletSelector(selector);
      setModal(walletModal);

      // Check if already signed in
      const state = selector.store.getState();
      if (state.accounts.length > 0) {
        setAccountId(state.accounts[0].accountId);
      }

      // Subscribe to account changes
      selector.store.observable.subscribe((state: { accounts: Array<{ accountId: string }> }) => {
        if (state.accounts.length > 0) {
          setAccountId(state.accounts[0].accountId);
        } else {
          setAccountId(null);
        }
      });
    };

    initWallet().catch(console.error);
  }, [config.networkId, config.contractId]);

  const call = async (methodName: string, args: Record<string, unknown>, options?: { attachedDeposit?: string; gas?: string }) => {
    if (!walletSelector) {
      throw new Error("Wallet selector not initialized");
    }

    const wallet = await walletSelector.wallet();
    const transactions = [{
      signerId: accountId!,
      receiverId: config.contractId,
      actions: [{
        type: "FunctionCall" as const,
        params: {
          methodName,
          args,
          gas: options?.gas || "30000000000000",
          deposit: options?.attachedDeposit || "0",
        }
      }]
    }];

    return wallet.signAndSendTransactions({ transactions });
  };

  const view = async <T,>(methodName: string, args: Record<string, unknown>): Promise<T> => {
    const response = await fetch(config.nodeUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: "dontcare",
        method: "query",
        params: {
          request_type: "call_function",
          account_id: config.contractId,
          method_name: methodName,
          args_base64: Buffer.from(JSON.stringify(args)).toString("base64"),
          finality: "optimistic",
        },
      }),
    });

    const { result, error } = await response.json();
    if (error) {
      throw new Error(error.message || "RPC call failed");
    }

    const decoded = Buffer.from(result.result).toString();
    return JSON.parse(decoded) as T;
  };

  const value: SimpleNearContextValue = {
    contractId: config.contractId,
    explorerUrl: config.explorerUrl,
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
    call,
    view,
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
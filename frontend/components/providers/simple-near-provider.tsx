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
  const [status, setStatus] = useState<"idle" | "loading" | "ready" | "error">("loading");
  const config = getNearConfig();

  useEffect(() => {
    let mounted = true;
    
    const initWallet = async () => {
      try {
        console.log('[SimpleNearProvider] Initializing wallet selector...');
        setStatus("loading");
        
        const selector = await setupWalletSelector({
          network: config.networkId as "testnet" | "mainnet",
          modules: [
            setupMyNearWallet(),
            setupMeteorWallet(),
            setupLedger(),
            setupNightly(),
          ],
        });

        if (!mounted) return;

        const walletModal = setupModal(selector, {
          contractId: config.contractId,
        });

        setWalletSelector(selector);
        setModal(walletModal);

        // Check if already signed in
        const state = selector.store.getState();
        console.log('[SimpleNearProvider] Initial wallet state:', state);
        
        if (state.accounts && state.accounts.length > 0) {
          console.log('[SimpleNearProvider] Found signed-in account:', state.accounts[0].accountId);
          setAccountId(state.accounts[0].accountId);
        }

        // Subscribe to account changes
        const subscription = selector.store.observable.subscribe((state: { accounts: Array<{ accountId: string }> }) => {
          console.log('[SimpleNearProvider] Wallet state changed:', state);
          if (state.accounts && state.accounts.length > 0) {
            setAccountId(state.accounts[0].accountId);
          } else {
            setAccountId(null);
          }
        });

        setStatus("ready");
        console.log('[SimpleNearProvider] Wallet selector ready');

        return () => {
          subscription.unsubscribe();
        };
      } catch (error) {
        console.error('[SimpleNearProvider] Failed to initialize wallet:', error);
        if (mounted) {
          setStatus("error");
        }
      }
    };

    initWallet();
    
    return () => {
      mounted = false;
    };
  }, [config.networkId, config.contractId]);

  const call = async (methodName: string, args: Record<string, unknown>, options?: { attachedDeposit?: string; gas?: string }) => {
    console.log(`[SimpleNearProvider] call: ${methodName}`, { args, options });
    
    if (!walletSelector) {
      throw new Error("Wallet selector not initialized");
    }
    if (!accountId) {
      throw new Error("No account signed in");
    }

    const wallet = await walletSelector.wallet();
    const transactions = [{
      signerId: accountId,
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

    console.log('[SimpleNearProvider] Sending transaction:', transactions);
    const result = await wallet.signAndSendTransactions({ transactions });
    console.log('[SimpleNearProvider] Transaction result:', result);
    return result;
  };

  const view = async <T,>(methodName: string, args: Record<string, unknown>): Promise<T> => {
    console.log(`[SimpleNearProvider] view call: ${methodName}`, args);
    
    // Use btoa instead of Buffer for browser compatibility
    const argsBase64 = btoa(JSON.stringify(args));
    
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
          args_base64: argsBase64,
          finality: "optimistic",
        },
      }),
    });

    if (!response.ok) {
      throw new Error(`RPC request failed: ${response.status} ${response.statusText}`);
    }

    const json = await response.json();
    console.log(`[SimpleNearProvider] RPC response for ${methodName}:`, json);
    
    if (json.error) {
      // Check for contract execution error
      if (json.error.cause && json.error.cause.name) {
        throw new Error(`Contract error: ${json.error.cause.name}`);
      }
      throw new Error(json.error.message || json.error.data || "RPC call failed");
    }

    // Handle missing result (contract doesn't exist or method failed)
    if (!json.result || !json.result.result) {
      console.warn(`[SimpleNearProvider] Missing result for ${methodName}`);
      return null as T;
    }

    // Handle empty result (null return from contract)
    if (json.result.result.length === 0) {
      console.log(`[SimpleNearProvider] Empty result (null) for ${methodName}`);
      return null as T;
    }

    // Decode the result using TextDecoder for browser compatibility
    const decoded = new TextDecoder().decode(new Uint8Array(json.result.result));
    console.log(`[SimpleNearProvider] Decoded result for ${methodName}:`, decoded);
    
    if (decoded === 'null' || decoded === '') {
      return null as T;
    }
    
    return JSON.parse(decoded) as T;
  };

  const value: SimpleNearContextValue = {
    contractId: config.contractId,
    explorerUrl: config.explorerUrl,
    signIn: async () => {
      if (!modal) {
        throw new Error("Wallet modal not initialized");
      }
      console.log('[SimpleNearProvider] Opening wallet modal...');
      modal.show();
    },
    signOut: async () => {
      if (!walletSelector) {
        throw new Error("Wallet selector not initialized");
      }
      console.log('[SimpleNearProvider] Signing out...');
      const wallet = await walletSelector.wallet();
      await wallet.signOut();
      setAccountId(null);
    },
    accountId,
    status,
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
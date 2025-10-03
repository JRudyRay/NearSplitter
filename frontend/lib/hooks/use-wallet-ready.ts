"use client";

import { useEffect, useMemo, useState } from "react";
import { useWalletSelector } from "@near-wallet-selector/react-hook";
import { getActiveAccount } from "@near-wallet-selector/core";
import type { WalletSelectorState } from "@near-wallet-selector/core";
import type { Wallet } from "@near-wallet-selector/core";

type StoreSubscription = { unsubscribe(): void } | null;

/**
 * Hook to check if wallet selector is fully initialized and ready to use.
 * 
 * This prevents race conditions where signedAccountId is set before
 * the wallet is actually ready to make RPC calls.
 * 
 * Returns:
 * - isReady: true when wallet selector is initialized and wallet instance exists
 * - accountId: The signed-in account ID (only when ready)
 * - wallet: The wallet instance (only when ready)
 */
export function useWalletReady() {
  const { signedAccountId, wallet, walletSelector } = useWalletSelector();
  const [isReady, setIsReady] = useState(false);
  const [readyAccountId, setReadyAccountId] = useState<string | null>(null);
  const [readyWallet, setReadyWallet] = useState<Wallet | null>(null);

  const [selectorAccountId, setSelectorAccountId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let subscription: StoreSubscription = null;

    async function syncFromStore() {
      try {
        if (!walletSelector) {
          setSelectorAccountId((prev) => (signedAccountId ? prev : null));
          return;
        }

        const selector = await walletSelector;
        if (cancelled) return;

        const updateFromState = (state: WalletSelectorState) => {
          if (cancelled) return;
          const activeAccount = getActiveAccount(state);
          setSelectorAccountId(activeAccount?.accountId ?? null);
        };

        updateFromState(selector.store.getState());
        subscription = selector.store.observable.subscribe(updateFromState);
      } catch (error) {
        console.error('[useWalletReady] Failed to sync wallet selector state:', error);
        if (!cancelled) {
          setSelectorAccountId(null);
        }
      }
    }

    if (signedAccountId) {
      setSelectorAccountId(signedAccountId);
      return () => {
        cancelled = true;
        subscription?.unsubscribe();
      };
    }

    syncFromStore();

    return () => {
      cancelled = true;
      subscription?.unsubscribe();
    };
  }, [signedAccountId, walletSelector]);

  const accountId = signedAccountId ?? selectorAccountId ?? null;
  const hasSignedAccount = Boolean(accountId);

  useEffect(() => {
    let mounted = true;

    async function checkWalletReady() {
      try {
        // Wait for wallet selector to be fully initialized
        const selector = walletSelector ? await walletSelector : null;

        if (!mounted || !selector) {
          setIsReady(false);
          setReadyAccountId(null);
          setReadyWallet(null);
          return;
        }

        const signedIn = selector.isSignedIn();

        if (!mounted || !signedIn || !accountId) {
          console.log('[useWalletReady] Wallet not ready yet:', {
            hasAccountId: Boolean(accountId),
            hasWallet: Boolean(wallet),
            signedIn,
          });

          setIsReady(false);
          setReadyAccountId(null);
          setReadyWallet(null);
          return;
        }

        const resolvedWallet = wallet ?? (await selector.wallet());

        if (!mounted) return;

        if (!resolvedWallet) {
          console.warn('[useWalletReady] Failed to resolve wallet instance despite active session.');
          setIsReady(false);
          setReadyAccountId(null);
          setReadyWallet(null);
          return;
        }

        console.log('[useWalletReady] Wallet is ready:', {
          accountId,
          hasWallet: Boolean(resolvedWallet)
        });

        setReadyWallet(resolvedWallet);
        setIsReady(true);
        setReadyAccountId(accountId);
      } catch (error) {
        console.error('[useWalletReady] Error checking wallet readiness:', error);
        if (mounted) {
          setIsReady(false);
          setReadyAccountId(null);
          setReadyWallet(null);
        }
      }
    }

    checkWalletReady();

    return () => {
      mounted = false;
    };
  }, [accountId, hasSignedAccount, wallet, walletSelector]);

  return {
    isReady,
    accountId: readyAccountId,
    wallet: useMemo(() => (isReady ? (readyWallet ?? wallet ?? null) : null), [isReady, readyWallet, wallet]),
  };
}

import { useEffect, useMemo, useState } from "react";
import { useWalletSelector } from "@near-wallet-selector/react-hook";
import { getActiveAccount } from "@near-wallet-selector/core";
import type { WalletSelectorState } from "@near-wallet-selector/core";

type StoreSubscription = { unsubscribe(): void } | null;

export function useNear() {
  const {
    signedAccountId,
    wallet,
    walletSelector,
    signIn,
    signOut,
    callFunction,
    viewFunction,
  } = useWalletSelector();

  const [derivedAccountId, setDerivedAccountId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
  let subscription: StoreSubscription = null;

    async function syncFromStore() {
      try {
        if (!walletSelector) {
          setDerivedAccountId((prev) => (signedAccountId ? prev : null));
          return;
        }

        const selector = await walletSelector;
        if (cancelled) return;

        const updateFromState = (state: WalletSelectorState) => {
          if (cancelled) return;
          const activeAccount = getActiveAccount(state);
          setDerivedAccountId(activeAccount?.accountId ?? null);
        };

        updateFromState(selector.store.getState());
        subscription = selector.store.observable.subscribe(updateFromState);
      } catch (error) {
        console.error('[useNear] Failed to sync wallet selector state:', error);
        if (!cancelled) {
          setDerivedAccountId(null);
        }
      }
    }

    // Prefer explicit account from hook context when available
    if (signedAccountId) {
      setDerivedAccountId(signedAccountId);
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

  const accountId = useMemo(() => signedAccountId ?? derivedAccountId ?? null, [signedAccountId, derivedAccountId]);
  const isSignedIn = Boolean(accountId);
  const status = isSignedIn ? "authenticated" : "unauthenticated";

  return {
    accountId,
    signedAccountId: accountId,
    signIn,
    signOut,
    callFunction,
    viewFunction,
    wallet,
    walletSelector,
    isSignedIn,
    status,
  };
}

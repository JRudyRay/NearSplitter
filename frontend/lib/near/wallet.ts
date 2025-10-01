"use client";

import { connect, keyStores, WalletConnection } from "near-api-js";
import { getNearConfig } from "@/lib/near/config";

let walletPromise: Promise<WalletConnection> | null = null;

async function createWalletConnection(): Promise<WalletConnection> {
  const { networkId, nodeUrl, walletUrl, helperUrl } = getNearConfig();

  const near = await connect({
    networkId,
    nodeUrl,
    walletUrl,
    helperUrl,
    deps: { keyStore: new keyStores.BrowserLocalStorageKeyStore() }
  });

  return new WalletConnection(near, "nearsplitter.app");
}

export async function initWallet(): Promise<WalletConnection> {
  if (typeof window === "undefined") {
    throw new Error("Wallet can only be initialised in the browser");
  }

  if (!walletPromise) {
    walletPromise = createWalletConnection();
  }

  return walletPromise;
}

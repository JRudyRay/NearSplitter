import { loadEnv } from "@/lib/env";

const NETWORK_MAP = {
  testnet: {
    networkId: "testnet",
    nodeUrl: "https://rpc.testnet.near.org",
    walletUrl: "https://app.mynearwallet.com",
    helperUrl: "https://helper.testnet.near.org",
    explorerUrl: "https://testnet.nearblocks.io"
  },
  mainnet: {
    networkId: "mainnet",
    nodeUrl: "https://rpc.mainnet.near.org",
    walletUrl: "https://app.mynearwallet.com",
    helperUrl: "https://helper.mainnet.near.org",
    explorerUrl: "https://nearblocks.io"
  }
} as const;

export type NetworkId = keyof typeof NETWORK_MAP;

export interface NearConfig {
  networkId: string;
  nodeUrl: string;
  walletUrl: string;
  helperUrl: string;
  explorerUrl: string;
  contractId: string;
}

export function getNearConfig(): NearConfig {
  const env = loadEnv();
  const networkKey = (env.NEXT_PUBLIC_NEAR_NETWORK ?? "testnet") as NetworkId;
  const base = NETWORK_MAP[networkKey];

  if (!base) {
    const keys = Object.keys(NETWORK_MAP).join(", ");
    throw new Error(`Unsupported NEAR network "${networkKey}". Expected one of: ${keys}`);
  }

  return {
    ...base,
    contractId: env.NEXT_PUBLIC_CONTRACT_ID
  };
}

import { loadEnv } from "@/lib/env";

const NETWORK_MAP = {
  testnet: {
    networkId: "testnet",
    nodeUrl: "https://rpc.testnet.fastnear.com",
    walletUrl: "https://app.mynearwallet.com",
    helperUrl: "https://helper.testnet.near.org",
    explorerUrl: "https://testnet.nearblocks.io",
    // Fallback RPC endpoints in case primary fails
    fallbackNodeUrls: [
      "https://test.rpc.fastnear.com",
      "https://rpc.testnet.pagoda.co",
      "https://rpc.testnet.near.org"
    ]
  },
  mainnet: {
    networkId: "mainnet",
    nodeUrl: "https://rpc.mainnet.near.org",
    walletUrl: "https://app.mynearwallet.com",
    helperUrl: "https://helper.mainnet.near.org",
    explorerUrl: "https://nearblocks.io",
    fallbackNodeUrls: [
      "https://rpc.mainnet.pagoda.co",
      "https://near-mainnet.lava.build",
      "https://mainnet.rpc.fastnear.com"
    ]
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
  fallbackNodeUrls: string[];
}

export function getNearConfig(): NearConfig {
  const env = loadEnv();
  const networkKey = (env.NEXT_PUBLIC_NEAR_NETWORK ?? "testnet") as NetworkId;
  const base = NETWORK_MAP[networkKey];

  if (!base) {
    const keys = Object.keys(NETWORK_MAP).join(", ");
    throw new Error(`Unsupported NEAR network "${networkKey}". Expected one of: ${keys}`);
  }

  // Allow overriding the RPC URL via environment variable
  const nodeUrl = env.NEXT_PUBLIC_NEAR_RPC_URL ?? base.nodeUrl;
  const fallbackNodeUrls = Array.from(
    new Set([
      // If we overrode the nodeUrl, keep the original base URL in the fallback list
      ...(env.NEXT_PUBLIC_NEAR_RPC_URL ? [base.nodeUrl] : []),
      ...base.fallbackNodeUrls,
    ])
  ).filter((url) => url && url !== nodeUrl);

  return {
    ...base,
    nodeUrl,
    fallbackNodeUrls,
    contractId: env.NEXT_PUBLIC_CONTRACT_ID
  };
}

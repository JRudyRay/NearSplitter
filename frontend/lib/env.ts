type EnvShape = {
  NEXT_PUBLIC_CONTRACT_ID: string;
  NEXT_PUBLIC_NEAR_NETWORK?: string;
  NEXT_PUBLIC_NEAR_RPC_URL?: string;
};

export function loadEnv(): EnvShape {
  // In Next.js 15 static exports, env vars must be inlined at build time
  // Reference them directly so they get replaced during build
  const contractId = process.env.NEXT_PUBLIC_CONTRACT_ID || 'nearsplitter-escrow.testnet';
  const network = process.env.NEXT_PUBLIC_NEAR_NETWORK || 'testnet';
  const rpcUrl = process.env.NEXT_PUBLIC_NEAR_RPC_URL;

  return {
    NEXT_PUBLIC_CONTRACT_ID: contractId,
    NEXT_PUBLIC_NEAR_NETWORK: network,
    NEXT_PUBLIC_NEAR_RPC_URL: rpcUrl,
  };
}

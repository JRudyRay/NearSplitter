type EnvShape = {
  NEXT_PUBLIC_CONTRACT_ID: string;
  NEXT_PUBLIC_NEAR_NETWORK?: string;
};

export function loadEnv(): EnvShape {
  // In Next.js 15 static exports, env vars must be inlined at build time
  // Reference them directly so they get replaced during build
  const contractId = process.env.NEXT_PUBLIC_CONTRACT_ID || 'nearsplitter-5134.testnet';
  const network = process.env.NEXT_PUBLIC_NEAR_NETWORK || 'testnet';

  return {
    NEXT_PUBLIC_CONTRACT_ID: contractId,
    NEXT_PUBLIC_NEAR_NETWORK: network,
  };
}

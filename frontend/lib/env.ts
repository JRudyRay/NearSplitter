type EnvShape = {
  NEXT_PUBLIC_CONTRACT_ID: string;
  NEXT_PUBLIC_NEAR_NETWORK?: string;
};

export function loadEnv(): EnvShape {
  // In Next.js 15, process.env must be accessed directly, not dynamically
  const contractId = process.env.NEXT_PUBLIC_CONTRACT_ID;
  const network = process.env.NEXT_PUBLIC_NEAR_NETWORK;

  if (!contractId) {
    throw new Error(
      `Missing required env var NEXT_PUBLIC_CONTRACT_ID. Add it to .env.local`
    );
  }

  return {
    NEXT_PUBLIC_CONTRACT_ID: contractId,
    NEXT_PUBLIC_NEAR_NETWORK: network,
  };
}

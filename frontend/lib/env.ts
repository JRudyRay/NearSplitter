const REQUIRED_ENV = ["NEXT_PUBLIC_CONTRACT_ID"] as const;

type RequiredEnvKeys = (typeof REQUIRED_ENV)[number];

type EnvShape = Record<RequiredEnvKeys, string> & {
  NEXT_PUBLIC_NEAR_NETWORK?: string;
};

export function loadEnv(): EnvShape {
  const values: Partial<EnvShape> = {};

  for (const key of REQUIRED_ENV) {
    const value = process.env[key];
    if (!value) {
      throw new Error(`Missing required env var ${key}. Add it to .env.local`);
    }
    values[key] = value;
  }

  if (process.env.NEXT_PUBLIC_NEAR_NETWORK) {
    values.NEXT_PUBLIC_NEAR_NETWORK = process.env.NEXT_PUBLIC_NEAR_NETWORK;
  }

  return values as EnvShape;
}

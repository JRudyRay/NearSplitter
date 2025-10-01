"use client";

import useSWR from "swr";
import { useNear } from "@/lib/hooks/use-near";

export function useContractView<T>(
  methodName: string | null,
  args: Record<string, unknown> | null,
  options?: { refreshInterval?: number }
) {
  const { viewFunction } = useNear();

  return useSWR<T>(
    methodName ? [methodName, JSON.stringify(args ?? {})] : null,
    async ([method, serializedArgs]: [string, string]) => {
      const parsedArgs = serializedArgs ? JSON.parse(serializedArgs) : {};
      return viewFunction({
        contractId: "your-contract.testnet", // TODO: Replace with actual contract ID
        method,
        args: parsedArgs,
      }) as Promise<T>;
    },
    {
      revalidateOnFocus: true,
      shouldRetryOnError: true,
      ...options
    }
  );
}

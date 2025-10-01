"use client";

import useSWR from "swr";
import { useNear } from "@/lib/hooks/use-near";
import { getNearConfig } from "@/lib/near/config";

export function useContractView<T>(
  methodName: string | null,
  args: Record<string, unknown> | null,
  options?: { refreshInterval?: number }
) {
  const { viewFunction } = useNear();
  const { contractId } = getNearConfig();

  return useSWR<T>(
    methodName ? [methodName, JSON.stringify(args ?? {})] : null,
    async ([method, serializedArgs]: [string, string]) => {
      const parsedArgs = serializedArgs ? JSON.parse(serializedArgs) : {};
      return viewFunction({
        contractId,
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

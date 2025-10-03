"use client";

import { useEffect, useState } from "react";
import useSWR from "swr";
import { viewFunction as rpcViewFunction } from "@/lib/near/rpc";

/**
 * Hook for making view calls to NEAR contracts.
 * 
 * Uses direct RPC calls (not wallet selector) because:
 * 1. View calls don't require wallet connection or authentication
 * 2. Works before wallet selector is initialized
 * 3. No "Exceeded 1 providers" errors
 * 4. More reliable for read-only operations
 * 
 * For transactions that change state, use useContractCall which uses wallet selector.
 * 
 * @param methodName - Contract method to call (null to disable)
 * @param args - Arguments for the contract method (null to disable)
 * @param options - SWR options and gate condition
 * @param options.refreshInterval - How often to refresh data
 * @param options.isReady - Optional gate: only fetch when true (for user-specific data)
 */
export function useContractView<T>(
  methodName: string | null,
  args: Record<string, unknown> | null,
  options?: { refreshInterval?: number; isReady?: boolean }
) {
  // Only enable on client side after mount
  const [isMounted, setIsMounted] = useState(false);
  
  useEffect(() => {
    setIsMounted(true);
  }, []);

  const { isReady = true, refreshInterval, ...swrOptions } = options ?? {};

  return useSWR<T>(
    // Only enable SWR when:
    // 1. Component is mounted
    // 2. Wallet/data is ready (if isReady gate provided)
    // 3. We have a method name
    // 4. We're in the browser
    isMounted && isReady && methodName && typeof window !== 'undefined' 
      ? [methodName, JSON.stringify(args ?? {})] 
      : null,
    async ([method]: [string, string]) => {
      const parsedArgs = args ?? {};
      
      // Debug logging
      console.log(`[useContractView] Calling ${method} with args:`, parsedArgs);
      
      try {
        // Use direct RPC for view calls - they don't need wallet authentication
        const result = await rpcViewFunction<T>(method, parsedArgs);
        
        console.log(`[useContractView] ${method} result:`, result);
        return result;
      } catch (error) {
        console.error(`[useContractView] Error calling ${method}:`, error);
        throw error;
      }
    },
    {
      revalidateOnFocus: false, // Don't auto-refetch on focus during sign-in
      revalidateOnReconnect: true,
      shouldRetryOnError: true,
      errorRetryCount: 3,
      errorRetryInterval: 1000,
      dedupingInterval: 2000, // Increase deduping to prevent rapid calls
      refreshInterval,
      ...swrOptions
    }
  );
}

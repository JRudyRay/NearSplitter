"use client";

import { useCallback, useState } from "react";
import type { FinalExecutionOutcome } from "near-api-js/lib/providers";
import { useNear } from "@/lib/hooks/use-near";

interface MutationState<T> {
  loading: boolean;
  data: T | null;
  error: Error | null;
}

interface CallOptions {
  attachedDeposit?: string;
  deposit?: string; // Alias for attachedDeposit
  gas?: string;
}

export function useContractCall() {
  const { callFunction } = useNear();
  const [state, setState] = useState<MutationState<FinalExecutionOutcome>>({
    loading: false,
    data: null,
    error: null
  });

  const execute = useCallback(
    async (methodName: string, args: Record<string, unknown>, options?: CallOptions) => {
      setState({ loading: true, data: null, error: null });
      try {
        const outcome = await callFunction({
          contractId: "your-contract.testnet", // TODO: Replace with actual contract ID
          method: methodName,
          args,
          gas: options?.gas,
          deposit: options?.deposit || options?.attachedDeposit,
        });
        setState({ loading: false, data: outcome as FinalExecutionOutcome, error: null });
        return outcome;
      } catch (error) {
        setState({ loading: false, data: null, error: error as Error });
        throw error;
      }
    },
    [callFunction]
  );

  return { ...state, execute };
}

"use client";

import { NearProvider } from "@/components/providers/near-provider";
import { ToastProvider } from "@/components/providers/toast-provider";

export function RootProviders({ children }: { children: React.ReactNode }) {
  return (
    <NearProvider>
      <ToastProvider>{children}</ToastProvider>
    </NearProvider>
  );
}

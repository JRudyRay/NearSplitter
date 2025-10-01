"use client";

import { SimpleNearProvider } from "@/components/providers/simple-near-provider";

export function RootProviders({ children }: { children: React.ReactNode }) {
  return <SimpleNearProvider>{children}</SimpleNearProvider>;
}

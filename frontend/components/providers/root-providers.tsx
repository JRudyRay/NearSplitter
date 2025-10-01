"use client";

import { NearProvider } from "@/components/providers/near-provider";

export function RootProviders({ children }: { children: React.ReactNode }) {
  return <NearProvider>{children}</NearProvider>;
}

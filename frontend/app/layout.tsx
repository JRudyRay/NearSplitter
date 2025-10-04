import type { Metadata } from "next";
import "./globals.css";
import { RootProviders } from "@/components/providers/root-providers";
import { ErrorBoundary } from "@/components/error-boundary";

export const metadata: Metadata = {
  title: "NearSplitter",
  description: "Split expenses on NEAR with native and token settlements"
};

export default function RootLayout({
  children
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" className="h-full">
      <body className="min-h-screen bg-slate-950 text-slate-100 antialiased">
        <ErrorBoundary>
          <RootProviders>{children}</RootProviders>
        </ErrorBoundary>
      </body>
    </html>
  );
}

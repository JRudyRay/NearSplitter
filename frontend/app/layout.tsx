import type { Metadata } from "next";
import "./globals.css";
import { RootProviders } from "@/components/providers/root-providers";
import { ErrorBoundary } from "@/components/error-boundary";
import { ThemeProvider } from "@/components/providers/theme-provider";

export const metadata: Metadata = {
  title: "NearSplitter - Split Expenses on NEAR",
  description: "The easiest way to split expenses with friends. Powered by NEAR Protocol for transparent, automatic settlements."
};

export default function RootLayout({
  children
}: {
  children: React.ReactNode;
}) {
  // Keep in sync with next.config.js basePath for GitHub Pages.
  const basePath = process.env.NODE_ENV === 'production' ? '/NearSplitter' : '';

  return (
    <html lang="en" className="h-full dark">
      <head>
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link rel="preconnect" href="https://fonts.gstatic.com" crossOrigin="anonymous" />
        <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&display=swap" rel="stylesheet" />

        {/* Explicit icon links so favicons work reliably with static export + basePath */}
        <link rel="icon" href={`${basePath}/icon/`} type="image/png" sizes="64x64" />
        <link rel="apple-touch-icon" href={`${basePath}/apple-icon/`} type="image/png" sizes="180x180" />
      </head>
      <body className="min-h-screen bg-bg text-fg antialiased">
        <ErrorBoundary>
          <ThemeProvider>
            <RootProviders>{children}</RootProviders>
          </ThemeProvider>
        </ErrorBoundary>
      </body>
    </html>
  );
}

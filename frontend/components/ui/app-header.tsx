'use client';

import { ArrowRight, HelpCircle, Wallet } from 'lucide-react';
import Link from 'next/link';
import { Logo } from '@/components/ui/logo';
import { Button } from '@/components/ui/button';
import { ThemeToggle } from '@/components/ui/theme-toggle';
import { useNear } from '@/lib/hooks/use-near';
import { useNearPrice } from '@/lib/hooks/use-near-price';

export function AppHeader({
  onConnect,
  onSignOut,
}: {
  onConnect: () => void;
  onSignOut: () => void;
}) {
  const near = useNear();
  const nearPrice = useNearPrice();

  const priceLabel = nearPrice ? nearPrice.usd.toFixed(2) : null;
  const freshness = nearPrice
    ? (() => {
        const ageMs = Date.now() - nearPrice.lastUpdated;
        if (ageMs < 60_000) return 'updated just now';
        const mins = Math.round(ageMs / 60_000);
        return `updated ${mins}m ago`;
      })()
    : null;

  return (
    <header className="near-card p-4" role="banner">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-3 min-w-0">
          <Link href="/" className="min-w-0" aria-label="Home">
            <Logo size="md" />
          </Link>
          
          {/* NEAR Price Display */}
          {priceLabel && (
            <div className="hidden sm:flex items-center gap-2 rounded-2xl px-3 py-2 border border-brand-500/40 bg-gradient-to-r from-brand-500/12 via-brand-500/6 to-transparent shadow-[0_8px_24px_rgba(0,0,0,0.18)] backdrop-blur-sm">
              <span className="inline-flex items-center gap-1 rounded-full bg-black/25 px-2 py-1 text-[11px] font-semibold text-brand-50 uppercase tracking-tight">
                <span className="h-2 w-2 rounded-full bg-emerald-400 animate-pulse" aria-hidden="true" />
                Live
              </span>
              <div className="flex items-baseline gap-1 text-sm font-semibold text-fg">
                <span className="text-muted-fg">Ⓝ</span>
                <span className="text-lg leading-none tracking-tight text-fg">${priceLabel}</span>
                <span className="text-[11px] text-muted-fg">USD</span>
              </div>
              {freshness && (
                <span className="text-[11px] text-muted-fg/80 whitespace-nowrap">{freshness}</span>
              )}
            </div>
          )}
        </div>

        <div className="flex items-center gap-2">
          <Link
            href="/help/"
            className="hidden sm:inline-flex items-center justify-center gap-2 font-medium px-4 py-2.5 text-base rounded-xl bg-muted hover:bg-muted/80 text-fg border border-border hover:border-brand-500/50 transition-all duration-200"
            aria-label="How it works"
          >
            <HelpCircle className="h-4 w-4 flex-shrink-0" />
            How It Works
          </Link>

          <ThemeToggle />

          {near.accountId ? (
            <div className="flex items-center gap-2">
              <span
                className="hidden sm:inline-flex rounded-xl bg-brand-500/10 px-3 py-2 text-sm font-semibold text-brand-500 ring-1 ring-brand-500/30 truncate max-w-[220px]"
                title={near.accountId}
              >
                {near.accountId}
              </span>
              <Button variant="secondary" size="md" onClick={onSignOut}>
                Sign out
              </Button>
            </div>
          ) : (
            <Button
              onClick={onConnect}
              size="md"
              leftIcon={<Wallet className="h-4 w-4" />}
              rightIcon={<ArrowRight className="h-4 w-4" />}
            >
              Connect wallet
            </Button>
          )}
        </div>
      </div>
    </header>
  );
}

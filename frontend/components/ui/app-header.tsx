'use client';

import { ArrowRight, HelpCircle, Wallet } from 'lucide-react';
import Link from 'next/link';
import { Logo } from '@/components/ui/logo';
import { Button } from '@/components/ui/button';
import { ThemeToggle } from '@/components/ui/theme-toggle';
import { useNear } from '@/lib/hooks/use-near';

export function AppHeader({
  onConnect,
  onSignOut,
}: {
  onConnect: () => void;
  onSignOut: () => void;
}) {
  const near = useNear();

  return (
    <header className="near-card p-4" role="banner">
      <div className="flex items-center justify-between gap-3">
        <Link href="/" className="min-w-0" aria-label="Home">
          <Logo size="md" />
        </Link>

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

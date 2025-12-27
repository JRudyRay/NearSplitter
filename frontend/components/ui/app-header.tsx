'use client';

import { ArrowRight, HelpCircle, Wallet } from 'lucide-react';
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
        <a href="/" className="min-w-0">
          <Logo size="md" />
        </a>

        <div className="flex items-center gap-2">
          <a
            href="/help"
            className="hidden sm:inline-flex"
            aria-label="How it works"
          >
            <Button variant="secondary" size="md" leftIcon={<HelpCircle className="h-4 w-4" />}
            >
              How It Works
            </Button>
          </a>

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

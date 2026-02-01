'use client';

import React from 'react';
import { Button } from '@/components/ui/button';

export function StorageRegistrationSection({
  isCheckingRegistration,
  isCheckingAfterReturn,
  accountId,
  requiredDepositLabel,
  requiredDepositValue,
  storageError,
  onRetryCheck,
  onRegister,
  registerLoading,
  disableRegister,
  storageBoundsError,
  onRetryStorageBounds,
}: {
  isCheckingRegistration: boolean;
  isCheckingAfterReturn?: boolean;
  accountId: string | null;
  requiredDepositLabel: string;
  requiredDepositValue: string;
  storageError: string | null;
  onRetryCheck: () => void;
  onRegister: () => void;
  registerLoading: boolean;
  disableRegister: boolean;
  storageBoundsError?: string | null;
  onRetryStorageBounds?: () => void;
}) {
  return (
    <section className="rounded-xl border-2 border-brand-500/50 bg-brand-500/10 p-3 sm:p-4 shadow-xl shadow-near-glow">
      {isCheckingRegistration ? (
        <div className="space-y-3">
          <div className="flex flex-col items-center justify-center gap-3 py-6">
            <div className="relative">
              <div className="h-12 w-12 animate-spin rounded-full border-3 border-brand-500/30 border-t-brand-500" />
              <div className="absolute inset-0 flex items-center justify-center">
                <svg className="h-5 w-5 text-brand-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                </svg>
              </div>
            </div>
            <div className="text-center">
              <p className="text-fg font-semibold text-xl">
                {isCheckingAfterReturn ? 'Confirming your registration...' : 'Checking registration status...'}
              </p>
              <p className="text-muted-fg mt-2 max-w-md">
                {isCheckingAfterReturn 
                  ? 'Your transaction is being finalized on the NEAR blockchain. This usually takes just a few seconds.'
                  : 'This may take a moment after completing registration.'}
              </p>
            </div>
          </div>
          <div className="flex justify-center gap-2">
            <Button onClick={onRetryCheck} variant="secondary" size="lg">
              {isCheckingAfterReturn ? 'Check Again' : 'Retry Check'}
            </Button>
          </div>
        </div>
      ) : (
        <div className="flex items-start gap-4">
          <div className={`rounded-full bg-brand-500/20 p-3 shadow-near-glow-sm`}>
            <svg className={`h-6 w-6 text-brand-500`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          </div>
          <div className="flex-1 space-y-2">
            <div>
              <h2 className="text-2xl font-bold text-fg">Registration Required</h2>
              <p className="mt-2 text-lg text-muted-fg">
                To use NearSplitter, you need to register once. This one-time deposit covers the storage cost for your account data on the
                NEAR blockchain.
              </p>
            </div>
            <div className="space-y-2 rounded-lg bg-bg/30 border border-border/60 p-4 text-lg">
              <div className="flex items-center justify-between">
                <span className="text-muted-fg">Registration status:</span>
                <span className="font-semibold text-rose-400">Not registered</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-muted-fg">{requiredDepositLabel}:</span>
                <span className={`font-semibold ${requiredDepositValue === 'Loading…' ? 'text-muted-fg' : 'text-brand-500'}`}>{requiredDepositValue}</span>
              </div>
              {accountId && (
                <div className="flex items-center justify-between">
                  <span className="text-muted-fg">Your account:</span>
                  <span className="font-semibold text-fg text-lg">{accountId}</span>
                </div>
              )}
              {storageError && (
                <div className="rounded bg-red-500/10 border border-red-500/30 p-2 mt-2">
                  <p className="text-lg text-red-400">Error checking registration: {storageError}</p>
                </div>
              )}
              {storageBoundsError && (
                <div className="rounded bg-orange-500/10 border border-orange-500/30 p-3 mt-2 space-y-2">
                  <p className="text-lg text-orange-400">⚠ Failed to load deposit amount: {storageBoundsError}</p>
                  {onRetryStorageBounds && (
                    <Button onClick={onRetryStorageBounds} variant="secondary" size="sm">
                      Retry Loading
                    </Button>
                  )}
                </div>
              )}
            </div>
            <Button
              onClick={onRegister}
              loading={registerLoading}
              disabled={disableRegister}
              className="font-semibold disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {registerLoading ? 'Registering…' : 'Register now'}
            </Button>
          </div>
        </div>
      )}
    </section>
  );
}

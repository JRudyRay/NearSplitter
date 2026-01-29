'use client';

import { AlertTriangle, X } from 'lucide-react';

interface PendingClaimsBannerProps {
  pendingCount: number;
  onDismiss?: () => void;
}

export function PendingClaimsBanner({ pendingCount, onDismiss }: PendingClaimsBannerProps) {
  if (pendingCount === 0) return null;

  return (
    <div className="rounded-lg border border-yellow-500/50 bg-yellow-500/10 p-3 mb-3">
      <div className="flex items-start gap-3">
        <div className="rounded-lg bg-yellow-500/20 p-1.5 shrink-0">
          <AlertTriangle className="h-4 w-4 text-yellow-500" />
        </div>
        <div className="flex-1">
          <h4 className="text-sm font-medium text-fg mb-0.5">
            {pendingCount} Pending Claim{pendingCount !== 1 ? 's' : ''}
          </h4>
          <p className="text-xs text-muted-fg">
            This circle cannot be settled until all claims are resolved.
            Payers must approve or reject claims on their expenses.
          </p>
        </div>
        {onDismiss && (
          <button
            onClick={onDismiss}
            className="rounded-lg p-1 hover:bg-muted transition-colors shrink-0"
          >
            <X className="h-4 w-4 text-muted-fg" />
          </button>
        )}
      </div>
    </div>
  );
}

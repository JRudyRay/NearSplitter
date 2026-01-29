'use client';

import { CheckCircle, XCircle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { ClaimBadge } from './claim-badge';
import type { Claim } from '@/lib/types';
import { formatNearAmount, formatTimestamp } from '@/lib/utils/format';

interface ClaimCardProps {
  claim: Claim;
  isPayer: boolean;
  onApprove?: (claimId: string) => void;
  onReject?: (claimId: string) => void;
  loading?: boolean;
}

const REASON_LABELS: Record<Claim['reason'], string> = {
  wrong_amount: 'Wrong Amount',
  wrong_participants: 'Wrong Participants',
  remove_expense: 'Remove Expense'
};

export function ClaimCard({ claim, isPayer, onApprove, onReject, loading = false }: ClaimCardProps) {
  return (
    <article className="rounded-lg border border-yellow-500/30 bg-yellow-500/5 p-3">
      <div className="flex items-start justify-between gap-2 mb-2">
        <div>
          <div className="flex items-center gap-2 mb-1">
            <span className="text-sm font-medium text-fg">{REASON_LABELS[claim.reason]}</span>
            <ClaimBadge status={claim.status} />
          </div>
          <p className="text-xs text-muted-fg">
            Filed by <span className="text-fg">{claim.claimant}</span> · {formatTimestamp(claim.created_ms)}
          </p>
        </div>
      </div>

      {/* Show proposed changes */}
      {claim.reason === 'wrong_amount' && claim.proposed_amount && (
        <div className="text-xs text-muted-fg mb-2">
          Proposed amount: <span className="text-brand-500 font-medium">{formatNearAmount(claim.proposed_amount)} Ⓝ</span>
        </div>
      )}

      {claim.reason === 'wrong_participants' && claim.proposed_participants && (
        <div className="text-xs text-muted-fg mb-2">
          <span className="block mb-1">Proposed participants:</span>
          <div className="flex flex-wrap gap-1">
            {claim.proposed_participants.map(p => (
              <span key={p.account_id} className="inline-flex items-center gap-1 rounded bg-muted px-1.5 py-0.5 text-xs">
                <span className="text-fg truncate max-w-[80px]">{p.account_id}</span>
                <span className="text-brand-500">{(p.weight_bps / 100).toFixed(0)}%</span>
              </span>
            ))}
          </div>
        </div>
      )}

      {claim.status !== 'pending' && claim.resolved_ms && (
        <p className="text-xs text-muted-fg">
          Resolved: {formatTimestamp(claim.resolved_ms)}
        </p>
      )}

      {/* Actions for payer only on pending claims */}
      {isPayer && claim.status === 'pending' && (
        <div className="flex gap-2 mt-3 pt-2 border-t border-border/50">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onReject?.(claim.id)}
            disabled={loading}
            className="flex-1 text-red-500 hover:bg-red-500/10"
          >
            <XCircle className="h-4 w-4 mr-1" />
            Reject
          </Button>
          <Button
            variant="primary"
            size="sm"
            onClick={() => onApprove?.(claim.id)}
            disabled={loading}
            className="flex-1"
          >
            <CheckCircle className="h-4 w-4 mr-1" />
            Approve
          </Button>
        </div>
      )}
    </article>
  );
}

'use client';

import { useState } from 'react';
import { AlertTriangle, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { ClaimReason } from '@/lib/types';

interface FileClaimModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (reason: ClaimReason, proposedAmount?: string, proposedParticipants?: Array<{ account_id: string; weight_bps: number }>) => Promise<void>;
  expenseMemo: string;
  expenseAmount: string;
  currentParticipants: Array<{ account_id: string; weight_bps: number }>;
  loading?: boolean;
}

const CLAIM_REASONS: { value: ClaimReason; label: string; description: string }[] = [
  {
    value: 'wrong_amount',
    label: 'Wrong Amount',
    description: 'The expense amount is incorrect'
  },
  {
    value: 'wrong_participants',
    label: 'Wrong Participants',
    description: 'The participant list or split is incorrect'
  },
  {
    value: 'remove_expense',
    label: 'Remove Expense',
    description: 'This expense should not exist'
  }
];

export function FileClaimModal({
  isOpen,
  onClose,
  onSubmit,
  expenseMemo,
  expenseAmount,
  currentParticipants,
  loading = false
}: FileClaimModalProps) {
  const [selectedReason, setSelectedReason] = useState<ClaimReason | null>(null);
  const [proposedAmount, setProposedAmount] = useState('');
  const [proposedParticipants, setProposedParticipants] = useState(
    currentParticipants.map(p => ({ ...p }))
  );
  const [submitError, setSubmitError] = useState<string | null>(null);

  const handleSubmit = async () => {
    if (!selectedReason) return;

    setSubmitError(null);

    let amount: string | undefined;
    let participants: Array<{ account_id: string; weight_bps: number }> | undefined;

    if (selectedReason === 'wrong_amount' && proposedAmount) {
      amount = proposedAmount;
    } else if (selectedReason === 'wrong_participants') {
      // Validate shares sum to 100% (10,000 bps)
      const totalBps = proposedParticipants.reduce((sum, p) => sum + p.weight_bps, 0);
      if (totalBps !== 10000) {
        setSubmitError(`Participant shares must sum to 100%. Currently: ${(totalBps / 100).toFixed(2)}%`);
        return;
      }
      participants = proposedParticipants;
    }

    try {
      await onSubmit(selectedReason, amount, participants);
      // Only close if submission succeeded
      handleClose();
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to file claim. Please try again.';
      setSubmitError(message);
    }
  };

  const handleClose = () => {
    setSelectedReason(null);
    setProposedAmount('');
    setProposedParticipants(currentParticipants.map(p => ({ ...p })));
    setSubmitError(null);
    onClose();
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/50 backdrop-blur-sm" onClick={handleClose} />
      
      {/* Modal */}
      <div className="relative z-10 w-full max-w-md rounded-xl border border-border bg-card p-4 shadow-lg mx-4">
        <div className="flex items-start justify-between mb-4">
          <div className="flex items-center gap-2">
            <div className="rounded-lg bg-yellow-500/20 p-1.5">
              <AlertTriangle className="h-4 w-4 text-yellow-500" />
            </div>
            <h2 className="text-lg font-bold text-fg">File a Claim</h2>
          </div>
          <button
            onClick={handleClose}
            className="rounded-lg p-1 hover:bg-muted transition-colors"
          >
            <X className="h-5 w-5 text-muted-fg" />
          </button>
        </div>

        <p className="text-sm text-muted-fg mb-4">
          Disputing expense: <span className="text-fg font-medium">{expenseMemo || 'Untitled'}</span>
          <span className="ml-2 text-brand-500 font-medium">{expenseAmount} Ⓝ</span>
        </p>

        {/* Error Display */}
        {submitError && (
          <div className="mb-4 rounded-lg bg-red-500/10 border border-red-500/30 p-3">
            <p className="text-sm text-red-600 dark:text-red-400">{submitError}</p>
          </div>
        )}

        {/* Reason Selection */}
        <div className="space-y-2 mb-4">
          <label className="text-sm font-semibold text-muted-fg">Select Reason</label>
          <div className="space-y-2">
            {CLAIM_REASONS.map(reason => (
              <button
                key={reason.value}
                type="button"
                onClick={() => setSelectedReason(reason.value)}
                className={`w-full text-left rounded-lg border p-3 transition-colors ${
                  selectedReason === reason.value
                    ? 'border-brand-500 bg-brand-500/10'
                    : 'border-border hover:border-border/80 hover:bg-muted/50'
                }`}
              >
                <div className="font-medium text-fg text-sm">{reason.label}</div>
                <div className="text-xs text-muted-fg">{reason.description}</div>
              </button>
            ))}
          </div>
        </div>

        {/* Conditional fields based on reason */}
        {selectedReason === 'wrong_amount' && (
          <div className="mb-4">
            <label className="text-sm font-semibold text-muted-fg mb-1.5 block">
              Proposed Correct Amount (Ⓝ)
            </label>
            <input
              type="number"
              step="0.01"
              min="0"
              value={proposedAmount}
              onChange={(e) => setProposedAmount(e.target.value)}
              placeholder="Enter correct amount"
              className="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-fg placeholder:text-muted-fg focus:border-brand-500 focus:outline-none focus:ring-1 focus:ring-brand-500"
            />
          </div>
        )}

        {selectedReason === 'wrong_participants' && (
          <div className="mb-4">
            <label className="text-sm font-semibold text-muted-fg mb-1.5 block">
              Proposed Participant Shares
            </label>
            <div className="space-y-2 max-h-40 overflow-y-auto">
              {proposedParticipants.map((participant, index) => (
                <div key={participant.account_id} className="flex items-center gap-2">
                  <span className="text-xs text-fg truncate flex-1">{participant.account_id}</span>
                  <input
                    type="number"
                    min="0"
                    max="100"
                    value={participant.weight_bps / 100}
                    onChange={(e) => {
                      const newParticipants = [...proposedParticipants];
                      newParticipants[index] = {
                        ...newParticipants[index],
                        weight_bps: Math.round(parseFloat(e.target.value || '0') * 100)
                      };
                      setProposedParticipants(newParticipants);
                    }}
                    className="w-16 rounded-lg border border-border bg-background px-2 py-1 text-xs text-fg text-right focus:border-brand-500 focus:outline-none focus:ring-1 focus:ring-brand-500"
                  />
                  <span className="text-xs text-muted-fg">%</span>
                </div>
              ))}
            </div>
            {(() => {
              const totalPercent = proposedParticipants.reduce((sum, p) => sum + p.weight_bps / 100, 0);
              const isValid = Math.abs(totalPercent - 100) < 0.01;
              return (
                <p className={`text-xs mt-1 ${isValid ? 'text-brand-500' : 'text-red-500'}`}>
                  Total: {totalPercent.toFixed(0)}% {!isValid && '(must equal 100%)'}
                </p>
              );
            })()}
          </div>
        )}

        {/* Actions */}
        <div className="flex gap-2">
          <Button
            variant="ghost"
            onClick={handleClose}
            className="flex-1"
            disabled={loading}
          >
            Cancel
          </Button>
          <Button
            variant="primary"
            onClick={handleSubmit}
            className="flex-1"
            disabled={!selectedReason || loading}
          >
            {loading ? 'Filing...' : 'File Claim'}
          </Button>
        </div>
      </div>
    </div>
  );
}

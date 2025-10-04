'use client';

import React, { type ReactNode } from 'react';
import { X, AlertTriangle, CheckCircle2, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils/cn';

interface ConfirmationModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void | Promise<void>;
  title: string;
  description?: string;
  confirmText?: string;
  cancelText?: string;
  type?: 'warning' | 'success' | 'info' | 'danger';
  loading?: boolean;
  children?: ReactNode;
  icon?: ReactNode;
  details?: Array<{ label: string; value: string | ReactNode }>;
}

export function ConfirmationModal({
  isOpen,
  onClose,
  onConfirm,
  title,
  description,
  confirmText = 'Confirm',
  cancelText = 'Cancel',
  type = 'info',
  loading = false,
  children,
  icon,
  details
}: ConfirmationModalProps) {
  const [isConfirming, setIsConfirming] = React.useState(false);

  const handleConfirm = async () => {
    setIsConfirming(true);
    try {
      await onConfirm();
    } finally {
      setIsConfirming(false);
    }
  };

  const handleBackdropClick = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget && !isConfirming && !loading) {
      onClose();
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape' && !isConfirming && !loading) {
      onClose();
    }
  };

  if (!isOpen) return null;

  const typeConfig = {
    warning: {
      iconBg: 'bg-yellow-500/20',
      iconRing: 'ring-yellow-500/10',
      iconColor: 'text-yellow-400',
      borderColor: 'border-yellow-900/50',
      gradientFrom: 'from-yellow-950/30',
      buttonClass: 'bg-yellow-500 hover:bg-yellow-600 text-black font-bold',
      defaultIcon: <AlertTriangle className="h-8 w-8" />
    },
    danger: {
      iconBg: 'bg-red-500/20',
      iconRing: 'ring-red-500/10',
      iconColor: 'text-red-400',
      borderColor: 'border-red-900/50',
      gradientFrom: 'from-red-950/30',
      buttonClass: 'bg-red-500 hover:bg-red-600 text-white font-bold',
      defaultIcon: <AlertTriangle className="h-8 w-8" />
    },
    success: {
      iconBg: 'bg-emerald-500/20',
      iconRing: 'ring-emerald-500/10',
      iconColor: 'text-emerald-400',
      borderColor: 'border-emerald-900/50',
      gradientFrom: 'from-emerald-950/30',
      buttonClass: 'bg-emerald-500 hover:bg-emerald-600 text-black font-bold',
      defaultIcon: <CheckCircle2 className="h-8 w-8" />
    },
    info: {
      iconBg: 'bg-blue-500/20',
      iconRing: 'ring-blue-500/10',
      iconColor: 'text-blue-400',
      borderColor: 'border-blue-900/50',
      gradientFrom: 'from-blue-950/30',
      buttonClass: 'bg-blue-500 hover:bg-blue-600 text-black font-bold',
      defaultIcon: <CheckCircle2 className="h-8 w-8" />
    }
  };

  const config = typeConfig[type];

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm animate-in fade-in duration-200"
      onClick={handleBackdropClick}
      onKeyDown={handleKeyDown}
      role="dialog"
      aria-modal="true"
      aria-labelledby="modal-title"
    >
      <div
        className={cn(
          "relative max-w-md w-full rounded-2xl border bg-gradient-to-br to-gray-950/50 p-6 shadow-2xl backdrop-blur-sm animate-in zoom-in-95 duration-200",
          config.borderColor,
          config.gradientFrom
        )}
      >
        {/* Close button */}
        <button
          onClick={onClose}
          disabled={isConfirming || loading}
          className="absolute top-4 right-4 p-2 rounded-lg text-gray-400 hover:text-white hover:bg-white/10 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          aria-label="Close modal"
        >
          <X className="h-5 w-5" />
        </button>

        {/* Icon */}
        <div className="flex justify-center mb-4">
          <div className={cn("rounded-full p-3 ring-4", config.iconBg, config.iconRing, config.iconColor)}>
            {icon || config.defaultIcon}
          </div>
        </div>

        {/* Title */}
        <h2 id="modal-title" className="text-2xl font-bold text-white text-center mb-2">
          {title}
        </h2>

        {/* Description */}
        {description && (
          <p className="text-gray-400 text-center mb-4">
            {description}
          </p>
        )}

        {/* Details */}
        {details && details.length > 0 && (
          <div className="my-4 rounded-lg bg-black/40 p-4 space-y-2 border border-gray-800">
            {details.map((detail, index) => (
              <div key={index} className="flex justify-between items-center text-sm">
                <span className="text-gray-400">{detail.label}:</span>
                <span className="text-white font-medium">{detail.value}</span>
              </div>
            ))}
          </div>
        )}

        {/* Custom content */}
        {children && (
          <div className="my-4">
            {children}
          </div>
        )}

        {/* Action buttons */}
        <div className="flex gap-3 mt-6">
          <Button
            variant="secondary"
            onClick={onClose}
            disabled={isConfirming || loading}
            className="flex-1"
          >
            {cancelText}
          </Button>
          <Button
            onClick={handleConfirm}
            disabled={isConfirming || loading}
            className={cn("flex-1", config.buttonClass)}
          >
            {(isConfirming || loading) && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
            {confirmText}
          </Button>
        </div>
      </div>
    </div>
  );
}

// Transaction-specific confirmation modal
interface TransactionConfirmationProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void | Promise<void>;
  transactionType: string;
  amount?: string;
  recipient?: string;
  gas?: string;
  loading?: boolean;
  additionalDetails?: Array<{ label: string; value: string | ReactNode }>;
}

export function TransactionConfirmation({
  isOpen,
  onClose,
  onConfirm,
  transactionType,
  amount,
  recipient,
  gas,
  loading,
  additionalDetails = []
}: TransactionConfirmationProps) {
  const details = [
    ...(amount ? [{ label: 'Amount', value: `${amount} Ⓝ` }] : []),
    ...(recipient ? [{ label: 'Recipient', value: recipient }] : []),
    ...(gas ? [{ label: 'Gas', value: gas }] : []),
    ...additionalDetails
  ];

  return (
    <ConfirmationModal
      isOpen={isOpen}
      onClose={onClose}
      onConfirm={onConfirm}
      title="Confirm Transaction"
      description={`You are about to ${transactionType}. This will require signing a transaction with your NEAR wallet.`}
      confirmText="Sign & Send"
      type="warning"
      loading={loading}
      details={details}
      icon={<AlertTriangle className="h-8 w-8" />}
    />
  );
}

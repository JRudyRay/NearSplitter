'use client';

import { AlertCircle, CheckCircle, XCircle } from 'lucide-react';
import type { Claim } from '@/lib/types';

interface ClaimBadgeProps {
  status: Claim['status'];
  className?: string;
}

const statusConfig = {
  pending: {
    icon: AlertCircle,
    label: 'Pending',
    className: 'bg-yellow-500/20 text-yellow-600 dark:text-yellow-400 border-yellow-500/30'
  },
  approved: {
    icon: CheckCircle,
    label: 'Approved',
    className: 'bg-green-500/20 text-green-600 dark:text-green-400 border-green-500/30'
  },
  rejected: {
    icon: XCircle,
    label: 'Rejected',
    className: 'bg-red-500/20 text-red-600 dark:text-red-400 border-red-500/30'
  }
} as const;

export function ClaimBadge({ status, className = '' }: ClaimBadgeProps) {
  const config = statusConfig[status];
  const Icon = config.icon;

  return (
    <span className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-xs font-medium ${config.className} ${className}`}>
      <Icon className="h-3 w-3" />
      {config.label}
    </span>
  );
}

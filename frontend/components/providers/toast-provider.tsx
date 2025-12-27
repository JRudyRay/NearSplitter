'use client';

import React, { createContext, useCallback, useContext, useMemo, useRef, useState } from 'react';
import { X, CheckCircle2, AlertTriangle, Info } from 'lucide-react';
import { cn } from '@/lib/utils/cn';

export type ToastTone = 'success' | 'error' | 'info';

type ToastItem = {
  id: string;
  tone: ToastTone;
  title?: string;
  message: string;
  actionLabel?: string;
  actionHref?: string;
  durationMs?: number;
};

type ToastContextValue = {
  push: (toast: Omit<ToastItem, 'id'>) => void;
  success: (message: string, opts?: Partial<Omit<ToastItem, 'id' | 'tone' | 'message'>>) => void;
  error: (message: string, opts?: Partial<Omit<ToastItem, 'id' | 'tone' | 'message'>>) => void;
  info: (message: string, opts?: Partial<Omit<ToastItem, 'id' | 'tone' | 'message'>>) => void;
};

const ToastContext = createContext<ToastContextValue | null>(null);

function randomId() {
  // Good enough for UI-only IDs
  return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const timeouts = useRef<Map<string, number>>(new Map());

  const dismiss = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
    const timeout = timeouts.current.get(id);
    if (timeout) {
      window.clearTimeout(timeout);
      timeouts.current.delete(id);
    }
  }, []);

  const push = useCallback((toast: Omit<ToastItem, 'id'>) => {
    const id = randomId();
    const durationMs = toast.durationMs ?? (toast.tone === 'error' ? 8_000 : 4_500);

    setToasts((prev) => [{ id, ...toast }, ...prev].slice(0, 3));

    const timeout = window.setTimeout(() => dismiss(id), durationMs);
    timeouts.current.set(id, timeout);
  }, [dismiss]);

  const api = useMemo<ToastContextValue>(() => {
    return {
      push,
      success: (message, opts) => push({ tone: 'success', message, ...opts }),
      error: (message, opts) => push({ tone: 'error', message, ...opts }),
      info: (message, opts) => push({ tone: 'info', message, ...opts }),
    };
  }, [push]);

  return (
    <ToastContext.Provider value={api}>
      {children}
      <div
        aria-live="polite"
        aria-relevant="additions"
        className="fixed inset-x-0 bottom-0 z-[60] px-4 pb-4 sm:left-auto sm:right-0 sm:w-[420px]"
      >
        <div className="flex flex-col gap-2">
          {toasts.map((t) => (
            <ToastRow key={t.id} toast={t} onDismiss={() => dismiss(t.id)} />
          ))}
        </div>
      </div>
    </ToastContext.Provider>
  );
}

export function useToast() {
  const ctx = useContext(ToastContext);
  if (!ctx) {
    throw new Error('useToast must be used within ToastProvider');
  }
  return ctx;
}

function ToastRow({ toast, onDismiss }: { toast: ToastItem; onDismiss: () => void }) {
  const tone = toast.tone;

  const toneStyles: Record<ToastTone, { ring: string; bg: string; fg: string; icon: React.ReactNode }> = {
    success: {
      ring: 'ring-1 ring-brand-500/30',
      bg: 'bg-card/95',
      fg: 'text-fg',
      icon: <CheckCircle2 className="h-4 w-4 text-brand-500" aria-hidden="true" />,
    },
    info: {
      ring: 'ring-1 ring-border',
      bg: 'bg-card/95',
      fg: 'text-fg',
      icon: <Info className="h-4 w-4 text-brand-500" aria-hidden="true" />,
    },
    error: {
      ring: 'ring-1 ring-red-500/40',
      bg: 'bg-card/95',
      fg: 'text-fg',
      icon: <AlertTriangle className="h-4 w-4 text-red-400" aria-hidden="true" />,
    },
  };

  const styles = toneStyles[tone];

  return (
    <div className={cn('near-card p-3 backdrop-blur-sm', styles.ring, styles.bg)}>
      <div className="flex items-start gap-3">
        <div className="mt-0.5">{styles.icon}</div>
        <div className={cn('min-w-0 flex-1', styles.fg)}>
          {toast.title && <div className="text-sm font-semibold truncate">{toast.title}</div>}
          <div className="text-sm text-muted-fg leading-snug">{toast.message}</div>
          {toast.actionHref && toast.actionLabel && (
            <div className="mt-2">
              <a
                className="inline-flex items-center rounded-lg px-2 py-1 text-xs font-semibold text-brand-500 hover:bg-brand-500/10"
                href={toast.actionHref}
                target="_blank"
                rel="noreferrer"
              >
                {toast.actionLabel}
              </a>
            </div>
          )}
        </div>
        <button
          type="button"
          onClick={onDismiss}
          className="rounded-lg p-1 text-muted-fg hover:text-fg hover:bg-muted/60"
          aria-label="Dismiss notification"
        >
          <X className="h-4 w-4" aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}

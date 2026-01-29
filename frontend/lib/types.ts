/**
 * Circle state machine - matches contract CircleState enum
 * UPDATED: Added to match contract state field for settlement tracking
 * - open: Normal operations
 * - settlement_in_progress: Confirmations in progress, circle locked
 * - settlement_executing: Autopay actively running (transient state)
 * - settled: Settlement complete, circle can be reactivated
 */
export type CircleState = 'open' | 'settlement_in_progress' | 'settlement_executing' | 'settled';

/** Valid circle state values - for runtime validation */
export const VALID_CIRCLE_STATES: readonly CircleState[] = [
  'open',
  'settlement_in_progress',
  'settlement_executing',
  'settled'
] as const;

/** Check if a string is a valid CircleState */
export function isValidCircleState(state: string): state is CircleState {
  return VALID_CIRCLE_STATES.includes(state as CircleState);
}

export interface Circle {
  id: string;
  owner: string;
  name: string;
  members: string[];
  created_ms: number;
  invite_code_hash?: string | null;
  locked: boolean;
  membership_open: boolean;
  /** UPDATED: Added state field to track settlement state machine */
  state: CircleState;
}

export interface MemberShare {
  account_id: string;
  weight_bps: number;
}

export interface Expense {
  id: string;
  circle_id: string;
  payer: string;
  participants: MemberShare[];
  amount_yocto: string;
  memo: string;
  ts_ms: number;
}

export interface Settlement {
  circle_id: string;
  from: string;
  to: string;
  amount: string;
  token: string | null;
  ts_ms: number;
  tx_kind: string;
}

/**
 * Balance view for a member in a circle
 * UPDATED: net is I128 which serializes to string (can be negative)
 */
export interface BalanceView {
  account_id: string;
  /** Net balance in yoctoNEAR as string (I128). Positive = creditor, negative = debtor */
  net: string;
}

export interface SettlementSuggestion {
  from: string;
  to: string;
  amount: string;
  token: string | null;
}

export interface StorageBalanceBounds {
  min: string;
  max: string | null;
}

export interface StorageBalance {
  total: string;
  available: string;
}

/** Claim filed by a participant to dispute an expense */
export interface Claim {
  id: string;
  circle_id: string;
  expense_id: string;
  claimant: string;
  /** Reason for the claim: "wrong_amount", "wrong_participants", "remove_expense" */
  reason: 'wrong_amount' | 'wrong_participants' | 'remove_expense';
  /** For "wrong_amount" claims: the proposed corrected amount in yoctoNEAR */
  proposed_amount?: string | null;
  /** For "wrong_participants" claims: the proposed new participant list */
  proposed_participants?: MemberShare[] | null;
  created_ms: number;
  /** Status: "pending", "approved", "rejected" */
  status: 'pending' | 'approved' | 'rejected';
  /** When the payer resolved the claim */
  resolved_ms?: number | null;
}

/** Reason options for filing a claim */
export type ClaimReason = 'wrong_amount' | 'wrong_participants' | 'remove_expense';

/** Valid claim reason values - for runtime validation */
export const VALID_CLAIM_REASONS: readonly ClaimReason[] = [
  'wrong_amount',
  'wrong_participants', 
  'remove_expense'
] as const;

/** Check if a string is a valid ClaimReason */
export function isValidClaimReason(reason: string): reason is ClaimReason {
  return VALID_CLAIM_REASONS.includes(reason as ClaimReason);
}

// ─── U128/I128 Normalization Utilities ─────────────────────────────────────────
// NEAR contract U128/I128 types serialize as strings in JSON
// These utilities normalize various response formats to consistent string representation

/**
 * Normalize a U128 value from contract response to string
 * Handles: string, number, bigint, { U128: string } formats
 * UPDATED: Added to handle all possible U128 serialization formats
 */
export function normalizeU128(value: unknown): string {
  if (value === null || value === undefined) {
    return '0';
  }
  if (typeof value === 'string') {
    return value;
  }
  if (typeof value === 'number' || typeof value === 'bigint') {
    return value.toString();
  }
  // Handle wrapped format { U128: "123" } or similar
  if (typeof value === 'object' && value !== null) {
    const obj = value as Record<string, unknown>;
    if ('U128' in obj && typeof obj.U128 === 'string') {
      return obj.U128;
    }
    if ('0' in obj && typeof obj['0'] === 'string') {
      return obj['0'];
    }
  }
  console.warn('[normalizeU128] Unexpected value format:', value);
  return '0';
}

/**
 * Normalize an I128 value from contract response to string
 * Similar to U128 but can represent negative values
 */
export function normalizeI128(value: unknown): string {
  if (value === null || value === undefined) {
    return '0';
  }
  if (typeof value === 'string') {
    return value;
  }
  if (typeof value === 'number' || typeof value === 'bigint') {
    return value.toString();
  }
  if (typeof value === 'object' && value !== null) {
    const obj = value as Record<string, unknown>;
    if ('I128' in obj && typeof obj.I128 === 'string') {
      return obj.I128;
    }
    if ('0' in obj && typeof obj['0'] === 'string') {
      return obj['0'];
    }
  }
  console.warn('[normalizeI128] Unexpected value format:', value);
  return '0';
}

/**
 * Safely parse a balance string to BigInt
 * Returns 0n on invalid input
 */
export function parseBalanceToBigInt(value: string | null | undefined): bigint {
  if (!value) return 0n;
  try {
    return BigInt(value);
  } catch {
    console.warn('[parseBalanceToBigInt] Invalid value:', value);
    return 0n;
  }
}

/**
 * Check if a balance is negative (debtor)
 */
export function isNegativeBalance(value: string | null | undefined): boolean {
  if (!value) return false;
  return value.startsWith('-');
}

/**
 * Get absolute value of a balance string
 */
export function absBalance(value: string | null | undefined): string {
  if (!value) return '0';
  return value.startsWith('-') ? value.slice(1) : value;
}

// ─── Runtime Validators ────────────────────────────────────────────────────────
// Type guards to validate contract response shapes at runtime

/**
 * Validate that a value matches Circle interface shape
 * UPDATED: Added state field validation
 */
export function isValidCircle(value: unknown): value is Circle {
  if (!value || typeof value !== 'object') return false;
  const obj = value as Record<string, unknown>;
  return (
    typeof obj.id === 'string' &&
    typeof obj.owner === 'string' &&
    typeof obj.name === 'string' &&
    Array.isArray(obj.members) &&
    typeof obj.created_ms === 'number' &&
    typeof obj.locked === 'boolean' &&
    typeof obj.membership_open === 'boolean' &&
    typeof obj.state === 'string' &&
    isValidCircleState(obj.state)
  );
}

/**
 * Validate that a value matches Expense interface shape
 */
export function isValidExpense(value: unknown): value is Expense {
  if (!value || typeof value !== 'object') return false;
  const obj = value as Record<string, unknown>;
  return (
    typeof obj.id === 'string' &&
    typeof obj.circle_id === 'string' &&
    typeof obj.payer === 'string' &&
    Array.isArray(obj.participants) &&
    (typeof obj.amount_yocto === 'string' || typeof obj.amount_yocto === 'object') &&
    typeof obj.memo === 'string' &&
    typeof obj.ts_ms === 'number'
  );
}

/**
 * Validate that a value matches Claim interface shape
 */
export function isValidClaim(value: unknown): value is Claim {
  if (!value || typeof value !== 'object') return false;
  const obj = value as Record<string, unknown>;
  return (
    typeof obj.id === 'string' &&
    typeof obj.circle_id === 'string' &&
    typeof obj.expense_id === 'string' &&
    typeof obj.claimant === 'string' &&
    typeof obj.reason === 'string' &&
    isValidClaimReason(obj.reason) &&
    typeof obj.status === 'string' &&
    typeof obj.created_ms === 'number'
  );
}

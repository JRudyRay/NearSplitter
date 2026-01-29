import {
  type BalanceView,
  type Circle,
  type Claim,
  type Expense,
  type Settlement,
  type SettlementSuggestion,
  type StorageBalance,
  type StorageBalanceBounds,
  normalizeU128,
  normalizeI128,
  isValidCircle,
  isValidExpense,
  isValidClaim
} from "@/lib/types";
import { getNearConfig } from "@/lib/near/config";

// Type for the viewFunction from wallet selector
type ViewFunction = (params: { contractId: string; method: string; args: Record<string, unknown> }) => Promise<unknown>;

// Helper to create a bound view function caller
function createViewCaller(viewFn: ViewFunction) {
  const { contractId } = getNearConfig();
  
  return async <T>(method: string, args: Record<string, unknown> = {}): Promise<T> => {
    return viewFn({ contractId, method, args }) as Promise<T>;
  };
}

/**
 * UPDATED: Added runtime validation for Circle response
 * Normalizes the state field which is now required
 */
export async function getCircle(circleId: string, viewFn: ViewFunction): Promise<Circle> {
  const view = createViewCaller(viewFn);
  const result = await view<unknown>("get_circle", { circle_id: circleId });
  
  // Runtime validation - ensure response has required state field
  if (!isValidCircle(result)) {
    console.warn('[getCircle] Response missing required fields, attempting normalization:', result);
    // Attempt to normalize old format (missing state field)
    const obj = result as Record<string, unknown>;
    if (obj && typeof obj === 'object' && !obj.state) {
      // Default to 'open' for backwards compatibility with pre-migration data
      (obj as Record<string, unknown>).state = obj.locked ? 'settlement_in_progress' : 'open';
    }
  }
  
  return result as Circle;
}

export async function listCirclesByOwner(owner: string, viewFn: ViewFunction, from = 0, limit = 50): Promise<Circle[]> {
  const view = createViewCaller(viewFn);
  const results = await view<unknown[]>("list_circles_by_owner", { owner, from, limit });
  // Normalize each circle
  return (results || []).map(normalizeCircleResponse);
}

/**
 * Helper to normalize Circle response with backwards compatibility
 */
function normalizeCircleResponse(result: unknown): Circle {
  if (!result || typeof result !== 'object') {
    throw new Error('Invalid circle response');
  }
  const obj = result as Record<string, unknown>;
  // Add default state if missing (backwards compatibility)
  if (!obj.state) {
    obj.state = obj.locked ? 'settlement_in_progress' : 'open';
  }
  // Cast through unknown to satisfy TypeScript strict mode
  return obj as unknown as Circle;
}

export async function listExpenses(circleId: string, viewFn: ViewFunction, from = 0, limit = 50): Promise<Expense[]> {
  const view = createViewCaller(viewFn);
  const results = await view<unknown[]>("list_expenses", { circle_id: circleId, from, limit });
  // UPDATED: Normalize U128 amount_yocto field in expenses
  return (results || []).map(normalizeExpenseResponse);
}

/**
 * Helper to normalize Expense response, especially U128 fields
 */
function normalizeExpenseResponse(result: unknown): Expense {
  if (!result || typeof result !== 'object') {
    throw new Error('Invalid expense response');
  }
  const obj = result as Record<string, unknown>;
  // Normalize amount_yocto from possible U128 wrapper to string
  obj.amount_yocto = normalizeU128(obj.amount_yocto);
  return obj as Expense;
}

/**
 * UPDATED: Normalize I128 net field in balance responses
 */
export async function computeBalances(circleId: string, viewFn: ViewFunction): Promise<BalanceView[]> {
  const view = createViewCaller(viewFn);
  const results = await view<unknown[]>("compute_balances", { circle_id: circleId });
  // Normalize I128 net field
  return (results || []).map((item) => {
    const obj = item as Record<string, unknown>;
    return {
      account_id: obj.account_id as string,
      net: normalizeI128(obj.net)
    };
  });
}

/**
 * UPDATED: Normalize U128 amount field in settlement suggestions
 */
export async function suggestSettlements(circleId: string, viewFn: ViewFunction): Promise<SettlementSuggestion[]> {
  const view = createViewCaller(viewFn);
  const results = await view<unknown[]>("suggest_settlements", { circle_id: circleId });
  return (results || []).map((item) => {
    const obj = item as Record<string, unknown>;
    return {
      from: obj.from as string,
      to: obj.to as string,
      amount: normalizeU128(obj.amount),
      token: obj.token as string | null
    };
  });
}

/**
 * UPDATED: Normalize U128 amount field in settlement history
 */
export async function listSettlements(circleId: string, viewFn: ViewFunction): Promise<Settlement[]> {
  const view = createViewCaller(viewFn);
  const results = await view<unknown[]>("list_settlements", { circle_id: circleId });
  return (results || []).map((item) => {
    const obj = item as Record<string, unknown>;
    return {
      circle_id: obj.circle_id as string,
      from: obj.from as string,
      to: obj.to as string,
      amount: normalizeU128(obj.amount),
      token: obj.token as string | null,
      ts_ms: obj.ts_ms as number,
      tx_kind: obj.tx_kind as string
    };
  });
}

/**
 * UPDATED: Normalize U128 values in storage balance bounds response
 */
export async function storageBalanceBounds(viewFn: ViewFunction): Promise<StorageBalanceBounds> {
  const view = createViewCaller(viewFn);
  const result = await view<unknown>("storage_balance_bounds");
  const obj = result as Record<string, unknown>;
  return {
    min: normalizeU128(obj.min),
    max: obj.max ? normalizeU128(obj.max) : null
  };
}

/**
 * UPDATED: Normalize U128 values in storage balance response
 */
export async function storageBalanceOf(accountId: string, viewFn: ViewFunction): Promise<StorageBalance | null> {
  const view = createViewCaller(viewFn);
  const result = await view<unknown>("storage_balance_of", { account_id: accountId });
  if (!result) return null;
  const obj = result as Record<string, unknown>;
  return {
    total: normalizeU128(obj.total),
    available: normalizeU128(obj.available)
  };
}

// Autopay and Confirmation Functions
export async function getConfirmations(circleId: string, viewFn: ViewFunction): Promise<string[]> {
  const view = createViewCaller(viewFn);
  return view<string[]>("get_confirmations", { circle_id: circleId });
}

export async function isFullyConfirmed(circleId: string, viewFn: ViewFunction): Promise<boolean> {
  const view = createViewCaller(viewFn);
  return view<boolean>("is_fully_confirmed", { circle_id: circleId });
}

export async function getAutopay(circleId: string, accountId: string, viewFn: ViewFunction): Promise<boolean> {
  const view = createViewCaller(viewFn);
  return view<boolean>("get_autopay", { circle_id: circleId, account_id: accountId });
}

export async function allMembersAutopay(circleId: string, viewFn: ViewFunction): Promise<boolean> {
  const view = createViewCaller(viewFn);
  return view<boolean>("all_members_autopay", { circle_id: circleId });
}

/**
 * UPDATED: Normalize U128 return value
 */
export async function getRequiredAutopayDeposit(circleId: string, accountId: string, viewFn: ViewFunction): Promise<string> {
  const view = createViewCaller(viewFn);
  const result = await view<unknown>("get_required_autopay_deposit", { circle_id: circleId, account_id: accountId });
  return normalizeU128(result);
}

/**
 * UPDATED: Normalize U128 return value
 */
export async function getEscrowDeposit(circleId: string, accountId: string, viewFn: ViewFunction): Promise<string> {
  const view = createViewCaller(viewFn);
  const result = await view<unknown>("get_escrow_deposit", { circle_id: circleId, account_id: accountId });
  return normalizeU128(result);
}

export async function isMembershipOpen(circleId: string, viewFn: ViewFunction): Promise<boolean> {
  const view = createViewCaller(viewFn);
  return view<boolean>("is_membership_open", { circle_id: circleId });
}

/**
 * UPDATED: Normalize U128 return value
 */
export async function getPendingPayout(accountId: string, viewFn: ViewFunction): Promise<string> {
  const view = createViewCaller(viewFn);
  const result = await view<unknown>("get_pending_payout", { account_id: accountId });
  return normalizeU128(result);
}

/**
 * UPDATED: Normalize circle responses with state field
 */
export async function listCirclesByMember(accountId: string, viewFn: ViewFunction, from = 0, limit = 50): Promise<Circle[]> {
  const view = createViewCaller(viewFn);
  const results = await view<unknown[]>("list_circles_by_member", { account_id: accountId, from, limit });
  return (results || []).map(normalizeCircleResponse);
}

// ─── Claims ────────────────────────────────────────────────────────────────────

/**
 * Helper to normalize Claim response, especially U128 proposed_amount
 */
function normalizeClaimResponse(result: unknown): Claim {
  if (!result || typeof result !== 'object') {
    throw new Error('Invalid claim response');
  }
  const obj = result as Record<string, unknown>;
  // Normalize proposed_amount from possible U128 wrapper
  if (obj.proposed_amount !== null && obj.proposed_amount !== undefined) {
    obj.proposed_amount = normalizeU128(obj.proposed_amount);
  }
  return obj as Claim;
}

/**
 * UPDATED: Normalize U128 fields in claims
 */
export async function listClaims(
  circleId: string,
  viewFn: ViewFunction,
  status?: 'pending' | 'approved' | 'rejected',
  from = 0,
  limit = 50
): Promise<Claim[]> {
  const view = createViewCaller(viewFn);
  const results = await view<unknown[]>("list_claims", { circle_id: circleId, status, from, limit });
  return (results || []).map(normalizeClaimResponse);
}

export async function getClaim(circleId: string, claimId: string, viewFn: ViewFunction): Promise<Claim | null> {
  const view = createViewCaller(viewFn);
  const result = await view<unknown>("get_claim", { circle_id: circleId, claim_id: claimId });
  if (!result) return null;
  return normalizeClaimResponse(result);
}

export async function getExpenseClaims(circleId: string, expenseId: string, viewFn: ViewFunction): Promise<Claim[]> {
  const view = createViewCaller(viewFn);
  const results = await view<unknown[]>("get_expense_claims", { circle_id: circleId, expense_id: expenseId });
  return (results || []).map(normalizeClaimResponse);
}

export async function getPendingClaimsCount(circleId: string, viewFn: ViewFunction): Promise<number> {
  const view = createViewCaller(viewFn);
  return view<number>("get_pending_claims_count", { circle_id: circleId });
}

export async function hasPendingClaims(circleId: string, viewFn: ViewFunction): Promise<boolean> {
  const view = createViewCaller(viewFn);
  return view<boolean>("has_pending_claims", { circle_id: circleId });
}

import {
  type BalanceView,
  type Circle,
  type Expense,
  type Settlement,
  type SettlementSuggestion,
  type StorageBalance,
  type StorageBalanceBounds
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

export async function getCircle(circleId: string, viewFn: ViewFunction): Promise<Circle> {
  const view = createViewCaller(viewFn);
  return view<Circle>("get_circle", { circle_id: circleId });
}

export async function listCirclesByOwner(owner: string, viewFn: ViewFunction, from = 0, limit = 50): Promise<Circle[]> {
  const view = createViewCaller(viewFn);
  return view<Circle[]>("list_circles_by_owner", { owner, from, limit });
}

export async function listExpenses(circleId: string, viewFn: ViewFunction, from = 0, limit = 50): Promise<Expense[]> {
  const view = createViewCaller(viewFn);
  return view<Expense[]>("list_expenses", { circle_id: circleId, from, limit });
}

export async function computeBalances(circleId: string, viewFn: ViewFunction): Promise<BalanceView[]> {
  const view = createViewCaller(viewFn);
  return view<BalanceView[]>("compute_balances", { circle_id: circleId });
}

export async function suggestSettlements(circleId: string, viewFn: ViewFunction): Promise<SettlementSuggestion[]> {
  const view = createViewCaller(viewFn);
  return view<SettlementSuggestion[]>("suggest_settlements", { circle_id: circleId });
}

export async function listSettlements(circleId: string, viewFn: ViewFunction): Promise<Settlement[]> {
  const view = createViewCaller(viewFn);
  return view<Settlement[]>("list_settlements", { circle_id: circleId });
}

export async function storageBalanceBounds(viewFn: ViewFunction): Promise<StorageBalanceBounds> {
  const view = createViewCaller(viewFn);
  return view<StorageBalanceBounds>("storage_balance_bounds");
}

export async function storageBalanceOf(accountId: string, viewFn: ViewFunction): Promise<StorageBalance | null> {
  const view = createViewCaller(viewFn);
  return view<StorageBalance | null>("storage_balance_of", { account_id: accountId });
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

export async function getRequiredAutopayDeposit(circleId: string, accountId: string, viewFn: ViewFunction): Promise<string> {
  const view = createViewCaller(viewFn);
  return view<string>("get_required_autopay_deposit", { circle_id: circleId, account_id: accountId });
}

export async function getEscrowDeposit(circleId: string, accountId: string, viewFn: ViewFunction): Promise<string> {
  const view = createViewCaller(viewFn);
  return view<string>("get_escrow_deposit", { circle_id: circleId, account_id: accountId });
}

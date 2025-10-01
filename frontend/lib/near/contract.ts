import {
  type BalanceView,
  type Circle,
  type Expense,
  type Settlement,
  type SettlementSuggestion,
  type StorageBalance,
  type StorageBalanceBounds
} from "@/lib/types";
import { viewFunction } from "@/lib/near/rpc";

export async function getCircle(circleId: string): Promise<Circle> {
  return viewFunction<Circle>("get_circle", { circle_id: circleId });
}

export async function listCirclesByOwner(owner: string, from = 0, limit = 50): Promise<Circle[]> {
  return viewFunction<Circle[]>("list_circles_by_owner", { owner, from, limit });
}

export async function listExpenses(circleId: string, from = 0, limit = 50): Promise<Expense[]> {
  return viewFunction<Expense[]>("list_expenses", { circle_id: circleId, from, limit });
}

export async function computeBalances(circleId: string): Promise<BalanceView[]> {
  return viewFunction<BalanceView[]>("compute_balances", { circle_id: circleId });
}

export async function suggestSettlements(circleId: string): Promise<SettlementSuggestion[]> {
  return viewFunction<SettlementSuggestion[]>("suggest_settlements", { circle_id: circleId });
}

export async function listSettlements(circleId: string): Promise<Settlement[]> {
  return viewFunction<Settlement[]>("list_settlements", { circle_id: circleId });
}

export async function storageBalanceBounds(): Promise<StorageBalanceBounds> {
  return viewFunction<StorageBalanceBounds>("storage_balance_bounds");
}

export async function storageBalanceOf(accountId: string): Promise<StorageBalance | null> {
  return viewFunction<StorageBalance | null>("storage_balance_of", { account_id: accountId });
}

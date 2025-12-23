export interface Circle {
  id: string;
  owner: string;
  name: string;
  members: string[];
  created_ms: number;
  invite_code_hash?: string | null;
  locked: boolean;
  membership_open: boolean;
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

export interface BalanceView {
  account_id: string;
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

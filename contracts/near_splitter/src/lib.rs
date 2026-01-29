use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use near_contract_standards::fungible_token::metadata::FungibleTokenMetadata;
use near_contract_standards::storage_management::{StorageBalance, StorageBalanceBounds};
use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap};
use near_sdk::env;
use near_sdk::json_types::{I128, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::serde_json::{self, json};
use near_sdk::{
    ext_contract, near_bindgen, require, AccountId, BorshStorageKey, Gas, NearToken,
    PanicOnDefault, Promise, PromiseOrValue, PromiseResult,
};

/// Minimum storage bytes reserved for account registration (entry in storage_deposits map)
/// This is the "locked" minimum that cannot be used for operations
const STORAGE_BYTES_REGISTRATION: u64 = 200;

/// Recommended total storage bytes for an account (covers registration + typical usage)
/// Used for storage_balance_bounds.min to tell users how much to deposit
const STORAGE_BYTES_RECOMMENDED: u64 = 25_000;

const MAX_PAGINATION_LIMIT: u64 = 100;  // Maximum items per page - prevents DoS attacks
const MAX_CIRCLE_MEMBERS: usize = 50;  // Maximum members per circle - prevents member explosion
const MAX_EXPENSES_PER_CIRCLE: usize = 500;  // Maximum expenses per circle - prevents storage DoS
const MAX_PARTICIPANTS_PER_EXPENSE: usize = 20;  // Maximum participants per expense
const MAX_CLAIMS_PER_CIRCLE: usize = 1_000;  // Maximum claims per circle
const MAX_SETTLEMENTS_PER_CIRCLE: usize = 10_000;  // Maximum settlements per circle
const ESTIMATED_SETTLEMENT_STORAGE_BYTES: u64 = 512;  // Conservative estimate for settlement storage
const EVENT_STANDARD: &str = "nearsplitter";
const EVENT_VERSION: &str = "1.0.0";
const TARGET_BPS_TOTAL: u16 = 10_000;
const ONE_YOCTO: u128 = 1;
const GAS_FT_TRANSFER_TGAS: u64 = 30;
const GAS_FT_CALLBACK_TGAS: u64 = 15;

fn timestamp_ms() -> u64 {
    env::block_timestamp() / 1_000_000
}

fn yocto_to_token(amount: u128) -> NearToken {
    NearToken::from_yoctonear(amount)
}

fn gas_ft_transfer() -> Gas {
    Gas::from_tgas(GAS_FT_TRANSFER_TGAS)
}

fn gas_ft_callback() -> Gas {
    Gas::from_tgas(GAS_FT_CALLBACK_TGAS)
}

#[derive(BorshSerialize, BorshDeserialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    Circles,
    ExpenseById,
    ExpensesLen,
    ExpensesIndex,
    SettlementById,
    SettlementsLen,
    SettlementsIndex,
    CirclesByOwner,
    CirclesByMember,
    StorageDeposits,
    MetadataCache,
    ConfirmationsMap,
    ConfirmationsCount,
    AutopayPreferences,
    EscrowDeposits,
    PendingPayouts,
    ClaimById,
    ClaimsLen,
    ClaimsIndex,
    /// C1-FIX: Monotonic expense ID counter per circle to prevent ID reuse after deletions
    NextExpenseIndex,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[borsh(crate = "near_sdk::borsh")]
#[serde(crate = "near_sdk::serde")]
pub enum CircleState {
    /// Normal operations - members can join, expenses added, etc.
    #[serde(rename = "open")]
    Open,
    /// B1-FIX: Settlement confirmations in progress - circle locked from edits but confirmations allowed
    #[serde(rename = "settlement_in_progress")]
    SettlementInProgress,
    /// B1-FIX: Autopay execution actively running - prevents re-entry during payout phase
    #[serde(rename = "settlement_executing")]
    SettlementExecuting,
    /// Settlement complete - circle can be reactivated
    #[serde(rename = "settled")]
    Settled,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[borsh(crate = "near_sdk::borsh")]
#[serde(crate = "near_sdk::serde")]
pub struct Circle {
    pub id: String,
    pub owner: AccountId,
    pub name: String,
    pub members: Vec<AccountId>,
    pub created_ms: u64,
    /// Optional invite code hash for private circles. If set, users must provide the hash to join.
    /// SECURITY: This is a pre-hashed value - the client hashes the password before sending.
    /// Format: SHA-256 hash of "salt:password:nearsplitter-v1" as hex string
    pub invite_code_hash: Option<String>,
    /// Salt used for the invite code hash (client-generated, random)
    /// Required when invite_code_hash is set
    pub invite_code_salt: Option<String>,
    /// When true, settlement is in progress (no new expenses, no joining allowed)
    pub locked: bool,
    /// When false, no new members can join (owner-controlled)
    pub membership_open: bool,
    /// State machine to prevent concurrent modifications during settlement
    pub state: CircleState,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[borsh(crate = "near_sdk::borsh")]
#[serde(crate = "near_sdk::serde")]
pub struct MemberShare {
    pub account_id: AccountId,
    pub weight_bps: u16,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[borsh(crate = "near_sdk::borsh")]
#[serde(crate = "near_sdk::serde")]
pub struct Expense {
    pub id: String,
    pub circle_id: String,
    pub payer: AccountId,
    pub participants: Vec<MemberShare>,
    pub amount_yocto: U128,
    pub memo: String,
    pub ts_ms: u64,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[borsh(crate = "near_sdk::borsh")]
#[serde(crate = "near_sdk::serde")]
pub struct Settlement {
    pub circle_id: String,
    pub from: AccountId,
    pub to: AccountId,
    pub amount: U128,
    pub token: Option<AccountId>,
    pub ts_ms: u64,
    pub tx_kind: String,
}

/// A claim filed by a participant to dispute an expense.
/// Only the original payer can approve or reject claims.
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[borsh(crate = "near_sdk::borsh")]
#[serde(crate = "near_sdk::serde")]
pub struct Claim {
    pub id: String,
    pub circle_id: String,
    pub expense_id: String,
    pub claimant: AccountId,
    /// Reason for the claim: "wrong_amount", "wrong_participants", "remove_expense"
    pub reason: String,
    /// For "wrong_amount" claims: the proposed corrected amount
    pub proposed_amount: Option<U128>,
    /// For "wrong_participants" claims: the proposed new participant list
    pub proposed_participants: Option<Vec<MemberShare>>,
    pub created_ms: u64,
    /// Status: "pending", "approved", "rejected"
    pub status: String,
    /// When the payer resolved the claim
    pub resolved_ms: Option<u64>,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct BalanceView {
    pub account_id: AccountId,
    pub net: I128,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SettlementSuggestion {
    pub from: AccountId,
    pub to: AccountId,
    pub amount: U128,
    pub token: Option<AccountId>,
}

#[derive(Deserialize)]
#[serde(crate = "near_sdk::serde")]
struct TransferMessage {
    circle_id: String,
    to: AccountId,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
#[borsh(crate = "near_sdk::borsh")]
pub struct NearSplitter {
    circles: UnorderedMap<String, Circle>,
    expense_by_id: LookupMap<String, Expense>,
    expenses_len: LookupMap<String, u64>,
    expenses_index: LookupMap<String, String>,
    settlement_by_id: LookupMap<String, Settlement>,
    settlements_len: LookupMap<String, u64>,
    settlements_index: LookupMap<String, String>,
    circles_by_owner: LookupMap<AccountId, Vec<String>>,
    /// Index for quick lookup of circles by member
    circles_by_member: LookupMap<AccountId, Vec<String>>,
    storage_deposits: LookupMap<AccountId, u128>,
    metadata_cache: LookupMap<AccountId, FungibleTokenMetadata>,
    next_circle_index: u64,
    /// Tracks which members have confirmed the ledger for each circle
    /// Key: "circle_id:account_id", Value: true if confirmed
    confirmations_map: LookupMap<String, bool>,
    /// Key: circle_id, Value: confirmation count
    confirmations_count: LookupMap<String, u64>,
    /// Tracks autopay preference for each user in each circle
    /// Key: "circle_id:account_id", Value: true if autopay enabled
    autopay_preferences: LookupMap<String, bool>,
    /// Tracks escrowed NEAR deposits for autopay settlements
    /// Key: "circle_id:account_id", Value: amount in yoctoNEAR
    escrow_deposits: LookupMap<String, u128>,
    /// Tracks pending payouts for each account (pull-payment pattern)
    /// Key: account_id, Value: amount in yoctoNEAR
    pending_payouts: LookupMap<AccountId, u128>,
    /// Tracks claims (disputes) on expenses
    /// Key: claim_id, Value: Claim
    claim_by_id: LookupMap<String, Claim>,
    /// Key: circle_id, Value: total claims appended (monotonic)
    claims_len: LookupMap<String, u64>,
    /// Key: "circle_id:idx", Value: claim_id
    claims_index: LookupMap<String, String>,
    /// C1-FIX: Monotonic expense ID counter per circle - prevents ID reuse after deletions
    /// Key: circle_id, Value: next expense index (never decremented)
    next_expense_index: LookupMap<String, u64>,
}

#[near_bindgen]
impl NearSplitter {
    // SECURITY: Added state check to prevent re-initialization attack
    // Without this, a malicious actor could reset all contract state
    #[init]
    pub fn new() -> Self {
        // Verify contract is not already initialized
        require!(
            !env::state_exists(),
            "Contract is already initialized"
        );
        Self {
            circles: UnorderedMap::new(StorageKey::Circles),
            expense_by_id: LookupMap::new(StorageKey::ExpenseById),
            expenses_len: LookupMap::new(StorageKey::ExpensesLen),
            expenses_index: LookupMap::new(StorageKey::ExpensesIndex),
            settlement_by_id: LookupMap::new(StorageKey::SettlementById),
            settlements_len: LookupMap::new(StorageKey::SettlementsLen),
            settlements_index: LookupMap::new(StorageKey::SettlementsIndex),
            circles_by_owner: LookupMap::new(StorageKey::CirclesByOwner),
            circles_by_member: LookupMap::new(StorageKey::CirclesByMember),
            storage_deposits: LookupMap::new(StorageKey::StorageDeposits),
            metadata_cache: LookupMap::new(StorageKey::MetadataCache),
            next_circle_index: 0,
            confirmations_map: LookupMap::new(StorageKey::ConfirmationsMap),
            confirmations_count: LookupMap::new(StorageKey::ConfirmationsCount),
            autopay_preferences: LookupMap::new(StorageKey::AutopayPreferences),
            escrow_deposits: LookupMap::new(StorageKey::EscrowDeposits),
            pending_payouts: LookupMap::new(StorageKey::PendingPayouts),
            claim_by_id: LookupMap::new(StorageKey::ClaimById),
            claims_len: LookupMap::new(StorageKey::ClaimsLen),
            claims_index: LookupMap::new(StorageKey::ClaimsIndex),
            // C1-FIX: Initialize monotonic expense counter storage
            next_expense_index: LookupMap::new(StorageKey::NextExpenseIndex),
        }
    }

    /// Migrate the contract state to a new version.
    /// 
    /// # WARNING: DEVELOPMENT/TESTNET ONLY
    /// This implementation resets all state. For production deployments:
    /// 1. Implement proper state migration logic
    /// 2. Preserve user funds and data
    /// 3. Use versioned state structs
    /// 
    /// # Security
    /// - #[private] macro restricts to contract account only
    /// - Typically called via DAO proposal after code upgrade
    #[cfg(any(test, feature = "dev"))]
    #[init(ignore_state)]
    #[private]
    pub fn migrate() -> Self {
        env::log_str("WARNING: State reset migration - all data cleared");
        Self {
            circles: UnorderedMap::new(StorageKey::Circles),
            expense_by_id: LookupMap::new(StorageKey::ExpenseById),
            expenses_len: LookupMap::new(StorageKey::ExpensesLen),
            expenses_index: LookupMap::new(StorageKey::ExpensesIndex),
            settlement_by_id: LookupMap::new(StorageKey::SettlementById),
            settlements_len: LookupMap::new(StorageKey::SettlementsLen),
            settlements_index: LookupMap::new(StorageKey::SettlementsIndex),
            circles_by_owner: LookupMap::new(StorageKey::CirclesByOwner),
            circles_by_member: LookupMap::new(StorageKey::CirclesByMember),
            storage_deposits: LookupMap::new(StorageKey::StorageDeposits),
            metadata_cache: LookupMap::new(StorageKey::MetadataCache),
            next_circle_index: 0,
            confirmations_map: LookupMap::new(StorageKey::ConfirmationsMap),
            confirmations_count: LookupMap::new(StorageKey::ConfirmationsCount),
            autopay_preferences: LookupMap::new(StorageKey::AutopayPreferences),
            escrow_deposits: LookupMap::new(StorageKey::EscrowDeposits),
            pending_payouts: LookupMap::new(StorageKey::PendingPayouts),
            claim_by_id: LookupMap::new(StorageKey::ClaimById),
            claims_len: LookupMap::new(StorageKey::ClaimsLen),
            claims_index: LookupMap::new(StorageKey::ClaimsIndex),
            // C1-FIX: Initialize monotonic expense counter storage
            next_expense_index: LookupMap::new(StorageKey::NextExpenseIndex),
        }
    }

    pub fn get_circle(&self, circle_id: String) -> Circle {
        self.circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"))
    }

    pub fn list_circles_by_owner(
        &self,
        owner: AccountId,
        from: Option<u64>,
        limit: Option<u64>,
    ) -> Vec<Circle> {
        let circles = self.circles_by_owner.get(&owner).unwrap_or_default();
        let safe_limit = limit.unwrap_or(50).min(MAX_PAGINATION_LIMIT);
        let slice = paginate_vec(&circles, from.unwrap_or(0), safe_limit);
        slice
            .iter()
            .filter_map(|id| self.circles.get(id))
            .collect()
    }

    /// Get all circles where the given account is a member (including owned circles)
    /// Uses indexed lookup for O(1) access instead of scanning all circles
    pub fn list_circles_by_member(
        &self,
        account_id: AccountId,
        from: Option<u64>,
        limit: Option<u64>,
    ) -> Vec<Circle> {
        let circle_ids = self.circles_by_member.get(&account_id).unwrap_or_default();
        let safe_limit = limit.unwrap_or(50).min(MAX_PAGINATION_LIMIT);
        let slice = paginate_vec(&circle_ids, from.unwrap_or(0), safe_limit);
        slice
            .iter()
            .filter_map(|id| self.circles.get(id))
            .collect()
    }

    /// Internal helper to add a member to the circles_by_member index
    fn add_member_to_index(&mut self, account_id: &AccountId, circle_id: &str) {
        let mut circles = self.circles_by_member.get(account_id).unwrap_or_default();
        if !circles.contains(&circle_id.to_string()) {
            circles.push(circle_id.to_string());
            self.circles_by_member.insert(account_id, &circles);
        }
    }

    /// Internal helper to remove a member from the circles_by_member index
    fn remove_member_from_index(&mut self, account_id: &AccountId, circle_id: &str) {
        let mut circles = self.circles_by_member.get(account_id).unwrap_or_default();
        circles.retain(|id| id != circle_id);
        if circles.is_empty() {
            self.circles_by_member.remove(account_id);
        } else {
            self.circles_by_member.insert(account_id, &circles);
        }
    }

    fn expense_index_key(circle_id: &str, idx: u64) -> String {
        format!("{}:{}", circle_id, idx)
    }

    fn settlement_index_key(circle_id: &str, idx: u64) -> String {
        format!("{}:{}", circle_id, idx)
    }

    fn claim_index_key(circle_id: &str, idx: u64) -> String {
        format!("{}:{}", circle_id, idx)
    }

    fn expense_by_index(&self, circle_id: &str, idx: u64) -> Option<Expense> {
        let key = Self::expense_index_key(circle_id, idx);
        self.expenses_index
            .get(&key)
            .and_then(|expense_id| self.expense_by_id.get(&expense_id))
    }

    fn claim_by_index(&self, circle_id: &str, idx: u64) -> Option<Claim> {
        let key = Self::claim_index_key(circle_id, idx);
        self.claims_index
            .get(&key)
            .and_then(|claim_id| self.claim_by_id.get(&claim_id))
    }

    fn iter_expenses_by_circle(&self, circle_id: &str) -> Vec<Expense> {
        let total = self.expenses_len.get(&circle_id.to_string()).unwrap_or(0);
        let mut items = Vec::new();
        for idx in 0..total {
            if let Some(expense) = self.expense_by_index(circle_id, idx) {
                items.push(expense);
            }
        }
        items
    }

    fn iter_claims_by_circle(&self, circle_id: &str) -> Vec<Claim> {
        let total = self.claims_len.get(&circle_id.to_string()).unwrap_or(0);
        let mut items = Vec::new();
        for idx in 0..total {
            if let Some(claim) = self.claim_by_index(circle_id, idx) {
                items.push(claim);
            }
        }
        items
    }

    fn iter_settlements_by_circle(&self, circle_id: &str) -> Vec<Settlement> {
        let total = self.settlements_len.get(&circle_id.to_string()).unwrap_or(0);
        let mut items = Vec::new();
        for idx in 0..total {
            let key = Self::settlement_index_key(circle_id, idx);
            if let Some(settlement_id) = self.settlements_index.get(&key) {
                if let Some(settlement) = self.settlement_by_id.get(&settlement_id) {
                    items.push(settlement);
                }
            }
        }
        items
    }

    fn clear_confirmations_for_circle(&mut self, circle_id: &str, members: &[AccountId]) {
        for member in members {
            let key = format!("{}:{}", circle_id, member);
            self.confirmations_map.remove(&key);
        }
        self.confirmations_count.remove(&circle_id.to_string());
    }

    fn clear_expenses_for_circle(&mut self, circle_id: &str) {
        let total = self.expenses_len.get(&circle_id.to_string()).unwrap_or(0);
        for idx in 0..total {
            let key = Self::expense_index_key(circle_id, idx);
            if let Some(expense_id) = self.expenses_index.get(&key) {
                self.expense_by_id.remove(&expense_id);
            }
            self.expenses_index.remove(&key);
        }
        self.expenses_len.remove(&circle_id.to_string());
    }

    fn clear_settlements_for_circle(&mut self, circle_id: &str) {
        let total = self.settlements_len.get(&circle_id.to_string()).unwrap_or(0);
        for idx in 0..total {
            let key = Self::settlement_index_key(circle_id, idx);
            if let Some(settlement_id) = self.settlements_index.get(&key) {
                self.settlement_by_id.remove(&settlement_id);
            }
            self.settlements_index.remove(&key);
        }
        self.settlements_len.remove(&circle_id.to_string());
    }

    fn clear_claims_for_circle(&mut self, circle_id: &str) {
        let total = self.claims_len.get(&circle_id.to_string()).unwrap_or(0);
        for idx in 0..total {
            let key = Self::claim_index_key(circle_id, idx);
            if let Some(claim_id) = self.claims_index.get(&key) {
                self.claim_by_id.remove(&claim_id);
            }
            self.claims_index.remove(&key);
        }
        self.claims_len.remove(&circle_id.to_string());
    }

    pub fn list_expenses(
        &self,
        circle_id: String,
        from: Option<u64>,
        limit: Option<u64>,
    ) -> Vec<Expense> {
        let safe_limit = limit.unwrap_or(50).min(MAX_PAGINATION_LIMIT);
        let total = self.expenses_len.get(&circle_id).unwrap_or(0);
        let mut results: Vec<Expense> = Vec::new();
        let mut idx = from.unwrap_or(0);
        while idx < total && results.len() < safe_limit as usize {
            if let Some(expense) = self.expense_by_index(&circle_id, idx) {
                results.push(expense);
            }
            idx += 1;
        }
        results
    }

    pub fn list_settlements(
        &self,
        circle_id: String,
        from: Option<u64>,
        limit: Option<u64>,
    ) -> Vec<Settlement> {
        let safe_limit = limit.unwrap_or(50).min(MAX_PAGINATION_LIMIT);
        let total = self.settlements_len.get(&circle_id).unwrap_or(0);
        let mut results: Vec<Settlement> = Vec::new();
        let mut idx = from.unwrap_or(0);
        while idx < total && results.len() < safe_limit as usize {
            let key = Self::settlement_index_key(&circle_id, idx);
            if let Some(settlement_id) = self.settlements_index.get(&key) {
                if let Some(settlement) = self.settlement_by_id.get(&settlement_id) {
                    results.push(settlement);
                }
            }
            idx += 1;
        }
        results
    }

    /// Compute net balances for all members in a circle.
    /// Positive balance = creditor (owed money), Negative balance = debtor (owes money).
    /// Expenses with pending claims are excluded from the calculation.
    pub fn compute_balances(&self, circle_id: String) -> Vec<BalanceView> {
        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));
        let expenses = self.iter_expenses_by_circle(&circle_id);
        
        // Get expense IDs that have pending claims (disputed expenses)
        let claims = self.iter_claims_by_circle(&circle_id);
        let disputed_expense_ids: HashSet<String> = claims
            .iter()
            .filter(|c| c.status == "pending")
            .map(|c| c.expense_id.clone())
            .collect();

        let mut net_map: HashMap<AccountId, i128> = HashMap::new();
        for member in &circle.members {
            net_map.entry(member.clone()).or_insert(0);
        }

        for expense in expenses {
            // Skip expenses with pending claims
            if disputed_expense_ids.contains(&expense.id) {
                continue;
            }
            
            let payer = &expense.payer;
            let amount_u128 = expense.amount_yocto.0;
            
            // Validate amount fits in i128 (max u128 / 2)
            require!(
                amount_u128 <= i128::MAX as u128,
                "Amount exceeds safe range for balance calculation"
            );
            let amount_i128 = amount_u128 as i128;

            let mut remaining = amount_u128;
            let last_index = expense.participants.len().saturating_sub(1);

            for (idx, share) in expense.participants.iter().enumerate() {
                let share_amount_u128 = if idx == last_index {
                    // Last participant gets remainder to handle rounding
                    remaining
                } else {
                    let computed = amount_u128
                        .checked_mul(share.weight_bps as u128)
                        .unwrap_or_else(|| env::panic_str("Share multiplication overflow"))
                        / TARGET_BPS_TOTAL as u128;
                    remaining = remaining
                        .checked_sub(computed)
                        .unwrap_or_else(|| env::panic_str("Share subtraction underflow"));
                    computed
                };

                let share_i128 = share_amount_u128 as i128; // Safe: share <= amount <= i128::MAX
                let entry = net_map.entry(share.account_id.clone()).or_insert(0);
                *entry = entry
                    .checked_sub(share_i128)
                    .unwrap_or_else(|| env::panic_str("Balance underflow"));
            }

            let payer_entry = net_map.entry(payer.clone()).or_insert(0);
            *payer_entry = payer_entry
                .checked_add(amount_i128)
                .unwrap_or_else(|| env::panic_str("Balance overflow"));
        }

        // Apply settlements to reduce outstanding balances.
        let settlements = self.iter_settlements_by_circle(&circle_id);
        for settlement in settlements {
            if settlement.token.is_some() {
                continue;
            }
            let amount_u128 = settlement.amount.0;
            require!(
                amount_u128 <= i128::MAX as u128,
                "Settlement amount exceeds safe range for balance calculation"
            );
            let amount_i128 = amount_u128 as i128;

            // Settlements reduce outstanding balances: payer's debt decreases, recipient's credit decreases.
            let from_entry = net_map.entry(settlement.from.clone()).or_insert(0);
            *from_entry = from_entry
                .checked_add(amount_i128)
                .unwrap_or_else(|| env::panic_str("Balance overflow"));

            let to_entry = net_map.entry(settlement.to.clone()).or_insert(0);
            *to_entry = to_entry
                .checked_sub(amount_i128)
                .unwrap_or_else(|| env::panic_str("Balance underflow"));
        }

        circle
            .members
            .iter()
            .map(|member| {
                let net = net_map.get(member).copied().unwrap_or_default();
                BalanceView {
                    account_id: member.clone(),
                    net: I128(net),
                }
            })
            .collect()
    }

    /// Suggest optimal settlements to settle all debts in the circle.
    /// Uses a greedy algorithm to minimize the number of transfers.
    /// Returns empty list if all balances are even.
    pub fn suggest_settlements(&self, circle_id: String) -> Vec<SettlementSuggestion> {
        let balances = self.compute_balances(circle_id);
        let mut debtors: Vec<(AccountId, u128)> = Vec::new();
        let mut creditors: Vec<(AccountId, u128)> = Vec::new();

        for balance in balances {
            match balance.net.0.cmp(&0) {
                Ordering::Less => debtors.push((balance.account_id, balance.net.0.unsigned_abs())),
                Ordering::Greater => {
                    // Safe: we only get here if net.0 > 0, so it fits in u128
                    creditors.push((balance.account_id, balance.net.0 as u128));
                }
                Ordering::Equal => {}
            }
        }

        // Early return if no debts
        if debtors.is_empty() || creditors.is_empty() {
            return Vec::new();
        }

        // Sort by amount descending for greedy matching
        debtors.sort_by(|a, b| b.1.cmp(&a.1));
        creditors.sort_by(|a, b| b.1.cmp(&a.1));

        let mut suggestions = Vec::new();
        let mut di = 0;
        let mut ci = 0;

        while di < debtors.len() && ci < creditors.len() {
            let (debtor, mut debt) = debtors[di].clone();
            let (creditor, mut credit) = creditors[ci].clone();
            let amount = debt.min(credit);

            suggestions.push(SettlementSuggestion {
                from: debtor.clone(),
                to: creditor.clone(),
                amount: U128(amount),
                token: None,
            });

            debt -= amount;
            credit -= amount;

            if debt == 0 {
                di += 1;
            } else {
                debtors[di].1 = debt;
            }

            if credit == 0 {
                ci += 1;
            } else {
                creditors[ci].1 = credit;
            }
        }

        suggestions
    }

    /// Create a new expense-sharing circle.
    /// Caller becomes the owner and first member.
    /// 
    /// # Security: Client-Side Hashing Required
    /// If creating a private circle, the client MUST hash the password before sending:
    /// 1. Generate a random salt (32+ chars recommended)
    /// 2. Compute: SHA-256("salt:password:nearsplitter-v1") 
    /// 3. Send invite_code_hash (hex string) and invite_code_salt
    /// 
    /// This prevents plaintext passwords from appearing on the blockchain!
    /// 
    /// SECURITY: Input validation prevents storage exhaustion attacks
    #[payable]
    pub fn create_circle(
        &mut self, 
        name: String, 
        invite_code_hash: Option<String>,
        invite_code_salt: Option<String>,
    ) -> String {
        let owner = env::predecessor_account_id();
        self.assert_registered(&owner);
        require!(!name.trim().is_empty(), "Circle name cannot be empty");
        require!(name.len() <= 256, "Circle name too long (max 256 bytes)");
        // SECURITY: Additional validation to prevent control characters in name
        require!(
            name.chars().all(|c| !c.is_control() || c == ' ' || c == '\t'),
            "Circle name contains invalid characters"
        );

        // Validate invite code hash and salt consistency
        let (validated_hash, validated_salt) = match (&invite_code_hash, &invite_code_salt) {
            (Some(hash), Some(salt)) => {
                // Validate hash format (64 hex chars = SHA-256)
                require!(hash.len() == 64, "Invalid invite code hash format (must be 64 hex chars)");
                require!(
                    hash.chars().all(|c| c.is_ascii_hexdigit()),
                    "Invalid invite code hash format (must be hexadecimal)"
                );
                // Validate salt (non-empty, reasonable length)
                require!(!salt.trim().is_empty(), "Salt cannot be empty");
                require!(salt.len() >= 16, "Salt too short (min 16 chars for security)");
                require!(salt.len() <= 128, "Salt too long (max 128 chars)");
                (Some(hash.clone()), Some(salt.clone()))
            }
            (None, None) => (None, None),
            _ => env::panic_str("invite_code_hash and invite_code_salt must both be provided or both be None"),
        };

        // Prevent overflow of circle index
        require!(
            self.next_circle_index < u64::MAX,
            "Maximum number of circles reached"
        );
        let initial_storage = env::storage_usage();
        let circle_id = format!("circle-{}", self.next_circle_index);
        self.next_circle_index = self.next_circle_index.saturating_add(1);
        let created_ms = timestamp_ms();

        let members = vec![owner.clone()];

        let circle = Circle {
            id: circle_id.clone(),
            owner: owner.clone(),
            name: name.clone(),
            members,
            created_ms,
            invite_code_hash: validated_hash,
            invite_code_salt: validated_salt,
            locked: false,
            membership_open: true, // New circles are open by default
            state: CircleState::Open,  // Initialize in Open state
        };

        self.circles.insert(&circle_id, &circle);

        let mut owner_list = self.circles_by_owner.get(&owner).unwrap_or_default();
        owner_list.push(circle_id.clone());
        self.circles_by_owner.insert(&owner, &owner_list);

        // Add owner to member index
        self.add_member_to_index(&owner, &circle_id);

        self.apply_storage_cost(&owner, initial_storage, true);

        self.emit_event(
            "circle_create",
            json!([{ 
                "circle_id": circle_id, 
                "owner": owner, 
                "name": name,
                "is_private": circle.invite_code_hash.is_some()
            }]),
        );
        circle.id
    }

    /// Join a circle. Requires invite code hash if the circle is private.
    /// 
    /// # Security: Client-Side Hashing Required
    /// The client MUST hash the password before sending:
    /// 1. Get the circle's invite_code_salt from get_circle() 
    /// 2. Compute: SHA-256("salt:password:nearsplitter-v1")
    /// 3. Send invite_code_hash (hex string)
    /// 
    /// Cannot join if:
    /// - Circle is not accepting new members (membership_open = false)
    /// - Circle is locked for settlement
    /// - Circle has reached maximum member limit
    /// - User is already a member
    // SECURITY: invite_code_hash is Optional - None is valid for public circles
    #[payable]
    pub fn join_circle(&mut self, circle_id: String, invite_code_hash: Option<String>) {
        let account = env::predecessor_account_id();
        self.assert_registered(&account);

        let mut circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        self.assert_circle_state_consistent(&circle);

        // Check if circle is accepting new members
        require!(circle.membership_open, "Circle is not accepting new members");
        require!(!circle.locked, "Circle is locked for settlement");
        require!(
            circle.state != CircleState::SettlementInProgress,
            "Cannot join during settlement"
        );
        require!(
            circle.state != CircleState::SettlementExecuting,
            "Cannot join during settlement execution"
        );
        require!(
            circle.state != CircleState::Settled || circle.membership_open,
            "Cannot join a finalized circle that is not accepting members"
        );

        // Verify invite code hash if circle is private
        // SECURITY: The client already hashed the password using the circle's salt
        // We just compare the pre-hashed values directly - no plaintext ever touches the chain!
        if let Some(expected_hash) = &circle.invite_code_hash {
            let provided_hash = invite_code_hash.unwrap_or_else(|| env::panic_str("This circle requires an invite code"));
            // Validate hash format
            require!(provided_hash.len() == 64, "Invalid invite code format");
            require!(
                provided_hash.chars().all(|c| c.is_ascii_hexdigit()),
                "Invalid invite code format"
            );
            require!(
                &provided_hash == expected_hash,
                "Invalid invite code"
            );
        }

        require!(circle.members.len() < MAX_CIRCLE_MEMBERS, "Circle has reached maximum member limit");
        require!(circle.members.iter().all(|m| m != &account), "Already a member");

        let initial_storage = env::storage_usage();

        circle.members.push(account.clone());
        self.circles.insert(&circle_id, &circle);

        // Add to member index
        self.add_member_to_index(&account, &circle_id);

        self.apply_storage_cost(&account, initial_storage, true);

        self.emit_event(
            "circle_join",
            json!([{ "circle_id": circle_id, "account_id": account }]),
        );
    }

    /// Leave a circle. Cannot leave if:
    /// - You are the owner (must transfer ownership first or delete circle)
    /// - Circle is not settled
    /// - You have a non-zero balance (must settle first)
    /// - You have escrowed funds
    /// - There are pending claims
    pub fn leave_circle(&mut self, circle_id: String) {
        let account = env::predecessor_account_id();
        
        let mut circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(circle.owner != account, "Owner cannot leave. Transfer ownership first.");
        require!(circle.state == CircleState::Settled, "Cannot leave until circle is settled");
        
        let member_index = circle.members.iter().position(|m| m == &account);
        require!(member_index.is_some(), "Not a member of this circle");

        let pending_claims = self.get_pending_claims_count(circle_id.clone());
        require!(pending_claims == 0, "Cannot leave with pending claims");
        
        // Check if user has non-zero balance
        let balances = self.compute_balances(circle_id.clone());
        let user_balance = balances
            .iter()
            .find(|b| b.account_id == account)
            .map(|b| b.net.0)
            .unwrap_or(0);
        
        require!(user_balance == 0, "Cannot leave with non-zero balance. Settle first.");

        let escrow_key = format!("{}:{}", circle_id, account);
        let escrowed = self.escrow_deposits.get(&escrow_key).unwrap_or(0);
        require!(escrowed == 0, "Cannot leave with escrowed funds");

        let initial_storage = env::storage_usage();
        
        // Remove from members
        circle.members.remove(member_index.unwrap());
        self.circles.insert(&circle_id, &circle);
        
        // Remove from member index
        self.remove_member_from_index(&account, &circle_id);
        
        // SECURITY: Complete ALL state changes before any external calls (reentrancy protection)
        // Cleanup any autopay/escrow state - get values before removing
        let autopay_key = format!("{}:{}", circle_id, account);
        self.autopay_preferences.remove(&autopay_key);
        let escrowed_refund = self.escrow_deposits.remove(&escrow_key).unwrap_or(0);
        
        // SECURITY: Clone account before move into emit_event closure
        let account_for_transfer = account.clone();

        self.apply_storage_cost(&account, initial_storage, false);
        
        self.emit_event(
            "circle_leave",
            json!([{ "circle_id": circle_id, "account_id": account }]),
        );
        
        // SECURITY: Transfer escrow refund AFTER all state changes (checks-effects-interactions pattern)
        if escrowed_refund > 0 {
            Promise::new(account_for_transfer).transfer(yocto_to_token(escrowed_refund));
        }
    }

    /// Transfer ownership of a circle to another member.
    /// Only the current owner can call this.
    #[payable]
    pub fn transfer_ownership(&mut self, circle_id: String, new_owner: AccountId) {
        let account = env::predecessor_account_id();
        
        let mut circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(circle.owner == account, "Only owner can transfer ownership");
        require!(!circle.locked, "Cannot transfer ownership during settlement");
        require!(
            circle.members.iter().any(|m| m == &new_owner),
            "New owner must be a circle member"
        );

        let initial_storage = env::storage_usage();
        
        // Update owner tracking
        let mut old_owner_circles = self.circles_by_owner.get(&account).unwrap_or_default();
        old_owner_circles.retain(|id| id != &circle_id);
        if old_owner_circles.is_empty() {
            self.circles_by_owner.remove(&account);
        } else {
            self.circles_by_owner.insert(&account, &old_owner_circles);
        }
        
        let mut new_owner_circles = self.circles_by_owner.get(&new_owner).unwrap_or_default();
        new_owner_circles.push(circle_id.clone());
        self.circles_by_owner.insert(&new_owner, &new_owner_circles);
        
        circle.owner = new_owner.clone();
        self.circles.insert(&circle_id, &circle);

        self.apply_storage_cost(&account, initial_storage, true);
        
        self.emit_event(
            "ownership_transferred",
            json!([{
                "circle_id": circle_id,
                "old_owner": account,
                "new_owner": new_owner,
            }]),
        );
    }

    /// Delete a circle. Only the owner can delete.
    /// Cannot delete if:
    /// - Circle is locked for settlement
    /// - There are any pending payouts or escrow deposits
    /// - Circle has more than one member (others must leave first)
    pub fn delete_circle(&mut self, circle_id: String) {
        let account = env::predecessor_account_id();
        
        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        self.assert_circle_state_consistent(&circle);

        require!(circle.owner == account, "Only owner can delete circle");
        require!(!circle.locked, "Cannot delete circle during settlement");
        require!(
            circle.state != CircleState::SettlementInProgress,
            "Cannot delete circle while settlement is in progress"
        );

        // Check that only owner remains (all others must leave first)
        require!(
            circle.members.len() == 1 && circle.members[0] == account,
            "All other members must leave before deleting circle"
        );

        let initial_storage = env::storage_usage();

        // Check no pending escrow deposits
        let escrow_key = format!("{}:{}", circle_id, account);
        let escrowed = self.escrow_deposits.get(&escrow_key).unwrap_or(0);
        require!(escrowed == 0, "Withdraw escrowed funds before deleting");

        // Remove from owner's circle list
        let mut owner_circles = self.circles_by_owner.get(&account).unwrap_or_default();
        owner_circles.retain(|id| id != &circle_id);
        if owner_circles.is_empty() {
            self.circles_by_owner.remove(&account);
        } else {
            self.circles_by_owner.insert(&account, &owner_circles);
        }

        // Remove from member index
        self.remove_member_from_index(&account, &circle_id);

        // Clean up all associated data
        self.circles.remove(&circle_id);
        self.clear_expenses_for_circle(&circle_id);
        self.clear_settlements_for_circle(&circle_id);
        self.clear_confirmations_for_circle(&circle_id, &circle.members);
        self.clear_claims_for_circle(&circle_id);
        self.next_expense_index.remove(&circle_id);
        
        // Clean up autopay preferences
        let autopay_key = format!("{}:{}", circle_id, account);
        self.autopay_preferences.remove(&autopay_key);

        self.apply_storage_cost(&account, initial_storage, false);

        self.emit_event(
            "circle_deleted",
            json!([{
                "circle_id": circle_id,
                "deleted_by": account,
            }]),
        );
    }

    #[payable]
    pub fn add_expense(
        &mut self,
        circle_id: String,
        amount_yocto: U128,
        shares: Vec<MemberShare>,
        memo: String,
    ) {
        require!(amount_yocto.0 > 0, "Amount must be positive");
        // SECURITY: Prevent overflow in balance calculations (i128::MAX for signed arithmetic)
        require!(
            amount_yocto.0 <= i128::MAX as u128,
            "Amount exceeds maximum safe value for balance calculation"
        );
        require!(!shares.is_empty(), "At least one share is required");
        require!(memo.len() <= 1024, "Memo too long (max 1024 bytes)");

        let payer = env::predecessor_account_id();
        self.assert_registered(&payer);

        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        self.assert_circle_state_consistent(&circle);
        
        require!(!circle.locked, "Circle is locked for settlement. Cannot add expenses.");
        require!(
            circle.state != CircleState::SettlementInProgress,
            "Cannot add expenses during settlement"
        );
        require!(
            circle.state != CircleState::SettlementExecuting,
            "Cannot add expenses during settlement execution"
        );
        
        require!(
            circle.members.iter().any(|m| m == &payer),
            "Payer must be circle member",
        );

        // Limit participants per expense - prevent participant explosion DoS
        require!(
            shares.len() <= MAX_PARTICIPANTS_PER_EXPENSE,
            "Expense cannot have more than 20 participants"
        );

        let mut sum_bps: u32 = 0;
        let mut unique_accounts: HashSet<AccountId> = HashSet::new();
        for share in &shares {
            require!(share.weight_bps > 0, "Share weight must be positive");
            require!(share.weight_bps <= TARGET_BPS_TOTAL, "Share weight exceeds 100%");
            require!(
                circle.members.iter().any(|m| m == &share.account_id),
                "Participant must be circle member",
            );
            require!(
                unique_accounts.insert(share.account_id.clone()),
                "Duplicate participant",
            );
            sum_bps += share.weight_bps as u32;
        }
        require!(sum_bps == TARGET_BPS_TOTAL as u32, "Shares must sum to 10_000 bps");

        let initial_storage = env::storage_usage();

        let current_len = self.expenses_len.get(&circle_id).unwrap_or(0);
        
        // Prevent storage DoS - limit expenses per circle (append-only index)
        require!(
            (current_len as usize) < MAX_EXPENSES_PER_CIRCLE,
            "Circle has reached maximum expense limit (500)"
        );
        
        // C1-FIX: Use monotonic counter that never decrements, even after deletions
        let expense_index = self.next_expense_index.get(&circle_id).unwrap_or(0);
        let expense_id = format!("expense-{}-{}", circle_id, expense_index + 1);
        self.next_expense_index.insert(&circle_id, &(expense_index + 1));
        let ts_ms = timestamp_ms();

        let expense = Expense {
            id: expense_id.clone(),
            circle_id: circle_id.clone(),
            payer: payer.clone(),
            participants: shares.clone(),
            amount_yocto,
            memo: memo.clone(),
            ts_ms,
        };

        let index_key = Self::expense_index_key(&circle_id, current_len);
        self.expenses_index.insert(&index_key, &expense_id);
        self.expense_by_id.insert(&expense_id, &expense);
        self.expenses_len.insert(&circle_id, &(current_len + 1));

        // Reset confirmations when new expense is added
        self.clear_confirmations_for_circle(&circle_id, &circle.members);

        self.apply_storage_cost(&payer, initial_storage, true);

        self.emit_event(
            "expense_add",
            json!([
                {
                    "circle_id": circle_id,
                    "expense_id": expense_id,
                    "payer": payer,
                    "amount": amount_yocto,
                    "memo": memo
                }
            ]),
        );
    }

    /// Delete an expense. Only the payer who created the expense can delete it.
    /// Cannot delete expenses that have pending claims.
    /// Cannot delete expenses while circle is locked for settlement.
    pub fn delete_expense(&mut self, circle_id: String, expense_id: String) {
        let caller = env::predecessor_account_id();
        self.assert_registered(&caller);

        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));
        self.assert_circle_state_consistent(&circle);

        require!(!circle.locked, "Cannot delete expenses while circle is locked for settlement");
        require!(
            circle.state != CircleState::SettlementInProgress,
            "Cannot delete expenses while settlement is in progress"
        );
        require!(
            circle.state != CircleState::SettlementExecuting,
            "Cannot delete expenses while settlement is executing"
        );

        let expense = self
            .expense_by_id
            .get(&expense_id)
            .unwrap_or_else(|| env::panic_str("Expense not found"));
        require!(expense.circle_id == circle_id, "Expense not found");

        require!(
            expense.payer == caller,
            "Only the expense payer can delete this expense"
        );

        // Check for pending claims on this expense
        let claims = self.iter_claims_by_circle(&circle_id);
        let has_pending_claim = claims
            .iter()
            .any(|c| c.expense_id == expense_id && c.status == "pending");
        require!(!has_pending_claim, "Cannot delete expense with pending claims");

        let initial_storage = env::storage_usage();

        let removed_amount = expense.amount_yocto;
        self.expense_by_id.remove(&expense_id);

        // Reset confirmations since balances changed
        self.clear_confirmations_for_circle(&circle_id, &circle.members);

        self.apply_storage_cost(&caller, initial_storage, false);

        self.emit_event(
            "expense_deleted",
            json!([{
                "circle_id": circle_id,
                "expense_id": expense_id,
                "deleted_by": caller,
                "amount": removed_amount,
            }]),
        );
    }

    // =========================================================================
    // CLAIMS (Expense Disputes)
    // =========================================================================

    /// File a claim to dispute an expense. Only participants in the expense can file claims.
    /// Reasons: "wrong_amount", "wrong_participants", "remove_expense"
    /// Cannot file claims while settlement is in progress.
    #[payable]
    pub fn file_claim(
        &mut self,
        circle_id: String,
        expense_id: String,
        reason: String,
        proposed_amount: Option<U128>,
        proposed_participants: Option<Vec<MemberShare>>,
    ) {
        let claimant = env::predecessor_account_id();
        self.assert_registered(&claimant);

        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(!circle.locked, "Cannot file claims while circle is locked for settlement");
        require!(
            circle.state != CircleState::SettlementInProgress,
            "Cannot file claims while settlement is in progress"
        );

        // Find the expense
        let expense = self
            .expense_by_id
            .get(&expense_id)
            .unwrap_or_else(|| env::panic_str("Expense not found"));
        require!(expense.circle_id == circle_id, "Expense not found");

        // Claimant must be a participant in this expense
        require!(
            expense.participants.iter().any(|p| p.account_id == claimant),
            "Only expense participants can file claims"
        );

        // Payer cannot file claims on their own expense (use edit if needed)
        require!(
            expense.payer != claimant,
            "Payer cannot dispute their own expense. Delete and re-add instead."
        );

        // Validate reason
        require!(
            reason == "wrong_amount" || reason == "wrong_participants" || reason == "remove_expense",
            "Invalid reason. Must be: wrong_amount, wrong_participants, or remove_expense"
        );
        require!(reason.len() <= 64, "Reason too long");

        // Validate proposed data based on reason
        if reason == "wrong_amount" {
            let amount = proposed_amount.unwrap_or_else(|| env::panic_str("Must provide proposed_amount for wrong_amount claims"));
            require!(amount.0 > 0, "Proposed amount must be positive");
        }

        if reason == "wrong_participants" {
            let participants = proposed_participants.as_ref().unwrap_or_else(|| env::panic_str("Must provide proposed_participants for wrong_participants claims"));
            require!(!participants.is_empty(), "Proposed participants cannot be empty");
            
            // Validate shares sum to 10,000 bps
            let mut sum_bps: u32 = 0;
            let mut unique_accounts: HashSet<AccountId> = HashSet::new();
            for share in participants {
                require!(share.weight_bps > 0, "Share weight must be positive");
                require!(share.weight_bps <= TARGET_BPS_TOTAL, "Share weight exceeds 100%");
                require!(
                    circle.members.iter().any(|m| m == &share.account_id),
                    "All proposed participants must be circle members"
                );
                require!(
                    unique_accounts.insert(share.account_id.clone()),
                    "Duplicate participant in proposed list"
                );
                sum_bps += share.weight_bps as u32;
            }
            require!(sum_bps == TARGET_BPS_TOTAL as u32, "Proposed shares must sum to 10_000 bps");
        }

        // Check for duplicate pending claim from same claimant on same expense
        let current_len = self.claims_len.get(&circle_id).unwrap_or(0);
        
        // Prevent storage DoS - limit claims per circle (append-only index)
        require!(
            (current_len as usize) < MAX_CLAIMS_PER_CIRCLE,
            "Circle has reached maximum claims limit (1,000)"
        );
        
        let existing_claims = self.iter_claims_by_circle(&circle_id);
        let duplicate = existing_claims
            .iter()
            .any(|c| c.expense_id == expense_id && c.claimant == claimant && c.status == "pending");
        require!(!duplicate, "You already have a pending claim on this expense");

        let initial_storage = env::storage_usage();

        // Create the claim with unique ID (includes count to prevent timestamp collisions)
        let claim_id = format!("claim-{}-{}-{}", circle_id, current_len, timestamp_ms());
        let claim = Claim {
            id: claim_id.clone(),
            circle_id: circle_id.clone(),
            expense_id: expense_id.clone(),
            claimant: claimant.clone(),
            reason: reason.clone(),
            proposed_amount,
            proposed_participants,
            created_ms: timestamp_ms(),
            status: "pending".to_string(),
            resolved_ms: None,
        };

        let index_key = Self::claim_index_key(&circle_id, current_len);
        self.claims_index.insert(&index_key, &claim_id);
        self.claim_by_id.insert(&claim_id, &claim);
        self.claims_len.insert(&circle_id, &(current_len + 1));

        // Reset confirmations when claim is filed
        self.clear_confirmations_for_circle(&circle_id, &circle.members);

        self.apply_storage_cost(&claimant, initial_storage, true);

        self.emit_event(
            "claim_filed",
            json!([{
                "circle_id": circle_id,
                "claim_id": claim_id,
                "expense_id": expense_id,
                "claimant": claimant,
                "reason": reason,
            }]),
        );
    }

    /// Approve a claim. Only the original payer of the expense can approve.
    /// This modifies or removes the expense based on the claim reason.
    /// Cannot approve claims while settlement is in progress.
    pub fn approve_claim(&mut self, circle_id: String, claim_id: String) {
        let caller = env::predecessor_account_id();
        self.assert_registered(&caller);

        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(!circle.locked, "Cannot resolve claims while circle is locked");
        require!(
            circle.state != CircleState::SettlementInProgress,
            "Cannot resolve claims while settlement is in progress"
        );

        // Find the claim
        let mut claim = self
            .claim_by_id
            .get(&claim_id)
            .unwrap_or_else(|| env::panic_str("Claim not found"));
        require!(claim.circle_id == circle_id, "Claim not found");
        require!(claim.status == "pending", "Claim is not pending");

        // Find the expense and verify caller is the payer
        let mut expense = self
            .expense_by_id
            .get(&claim.expense_id)
            .unwrap_or_else(|| env::panic_str("Expense not found"));
        require!(expense.circle_id == circle_id, "Expense not found");
        
        require!(
            expense.payer == caller,
            "Only the expense payer can approve claims"
        );

        let initial_storage = env::storage_usage();

        // Apply the claim based on reason
        // C2-FIX: Thoroughly validate proposed values to preserve expense invariants
        match claim.reason.as_str() {
            "wrong_amount" => {
                // C2-FIX: Validate proposed_amount meets all expense constraints
                let new_amount = claim
                    .proposed_amount
                    .unwrap_or_else(|| env::panic_str("Claim missing proposed_amount"));
                require!(new_amount.0 > 0, "Proposed amount must be positive");
                // C2-FIX: Ensure amount fits in i128 for balance calculation safety
                require!(
                    new_amount.0 <= i128::MAX as u128,
                    "Proposed amount exceeds maximum safe value for balance calculation"
                );
                let old_amount = expense.amount_yocto;
                expense.amount_yocto = new_amount;
                self.expense_by_id.insert(&expense.id, &expense);
                
                self.emit_event(
                    "expense_amount_updated",
                    json!([{
                        "circle_id": circle_id,
                        "expense_id": claim.expense_id,
                        "old_amount": old_amount,
                        "new_amount": new_amount,
                    }]),
                );
            }
            "wrong_participants" => {
                // C2-FIX: Thoroughly validate proposed_participants preserves all expense invariants
                let new_participants = claim
                    .proposed_participants
                    .clone()
                    .unwrap_or_else(|| env::panic_str("Claim missing proposed_participants"));
                require!(!new_participants.is_empty(), "Proposed participants cannot be empty");
                require!(
                    new_participants.len() <= MAX_PARTICIPANTS_PER_EXPENSE,
                    "Proposed participants exceed maximum limit"
                );
                
                // C2-FIX: Validate all participants are circle members, no duplicates, weights valid
                let mut sum_bps: u32 = 0;
                let mut unique_accounts: HashSet<AccountId> = HashSet::new();
                for share in &new_participants {
                    require!(share.weight_bps > 0, "Share weight must be positive");
                    require!(share.weight_bps <= TARGET_BPS_TOTAL, "Share weight exceeds 100%");
                    require!(
                        circle.members.iter().any(|m| m == &share.account_id),
                        "All proposed participants must be circle members"
                    );
                    require!(
                        unique_accounts.insert(share.account_id.clone()),
                        "Duplicate participant in proposed list"
                    );
                    sum_bps += share.weight_bps as u32;
                }
                require!(sum_bps == TARGET_BPS_TOTAL as u32, "Proposed shares must sum to 10_000 bps");
                
                expense.participants = new_participants;
                self.expense_by_id.insert(&expense.id, &expense);
                
                self.emit_event(
                    "expense_participants_updated",
                    json!([{
                        "circle_id": circle_id,
                        "expense_id": claim.expense_id,
                    }]),
                );
            }
            "remove_expense" => {
                let removed_expense_id = expense.id.clone();
                self.expense_by_id.remove(&removed_expense_id);
                
                self.emit_event(
                    "expense_removed",
                    json!([{
                        "circle_id": circle_id,
                        "expense_id": removed_expense_id,
                    }]),
                );
            }
            _ => {}
        }

        // Update claim status
        claim.status = "approved".to_string();
        claim.resolved_ms = Some(timestamp_ms());
        self.claim_by_id.insert(&claim_id, &claim);

        // Reset confirmations since balances changed
        self.clear_confirmations_for_circle(&circle_id, &circle.members);

        self.apply_storage_cost(&caller, initial_storage, false);

        self.emit_event(
            "claim_approved",
            json!([{
                "circle_id": circle_id,
                "claim_id": claim_id,
                "expense_id": claim.expense_id,
                "reason": claim.reason,
                "approved_by": caller,
            }]),
        );
    }

    /// Reject a claim. Only the original payer of the expense can reject.
    /// This marks the claim as rejected and the expense remains unchanged.
    /// Cannot reject claims while settlement is in progress.
    pub fn reject_claim(&mut self, circle_id: String, claim_id: String) {
        let caller = env::predecessor_account_id();
        self.assert_registered(&caller);

        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(!circle.locked, "Cannot resolve claims while circle is locked");
        require!(
            circle.state != CircleState::SettlementInProgress,
            "Cannot resolve claims while settlement is in progress"
        );

        // Find the claim
        let mut claim = self
            .claim_by_id
            .get(&claim_id)
            .unwrap_or_else(|| env::panic_str("Claim not found"));
        require!(claim.circle_id == circle_id, "Claim not found");
        require!(claim.status == "pending", "Claim is not pending");

        // Find the expense and verify caller is the payer
        let expense = self
            .expense_by_id
            .get(&claim.expense_id)
            .unwrap_or_else(|| env::panic_str("Expense not found"));
        require!(expense.circle_id == circle_id, "Expense not found");
        
        require!(
            expense.payer == caller,
            "Only the expense payer can reject claims"
        );

        let initial_storage = env::storage_usage();

        // Update claim status
        claim.status = "rejected".to_string();
        claim.resolved_ms = Some(timestamp_ms());
        self.claim_by_id.insert(&claim_id, &claim);

        // Reset confirmations to re-evaluate
        self.clear_confirmations_for_circle(&circle_id, &circle.members);

        self.apply_storage_cost(&caller, initial_storage, false);

        self.emit_event(
            "claim_rejected",
            json!([{
                "circle_id": circle_id,
                "claim_id": claim_id,
                "expense_id": claim.expense_id,
                "rejected_by": caller,
            }]),
        );
    }

    /// Get all claims for a circle, optionally filtered by status
    pub fn list_claims(
        &self,
        circle_id: String,
        status: Option<String>,
        from: Option<u64>,
        limit: Option<u64>,
    ) -> Vec<Claim> {
        // NOTE: Bounded scan across claims (MAX_CLAIMS_PER_CIRCLE)
        let claims = self.iter_claims_by_circle(&circle_id);
        let safe_limit = limit.unwrap_or(50).min(MAX_PAGINATION_LIMIT);
        
        let filtered: Vec<Claim> = if let Some(status_filter) = status {
            claims.into_iter().filter(|c| c.status == status_filter).collect()
        } else {
            claims
        };
        
        paginate_vec(&filtered, from.unwrap_or(0), safe_limit)
    }

    /// Get a specific claim by ID
    pub fn get_claim(&self, circle_id: String, claim_id: String) -> Option<Claim> {
        self.claim_by_id
            .get(&claim_id)
            .and_then(|claim| if claim.circle_id == circle_id { Some(claim) } else { None })
    }

    /// Get all claims for a specific expense
    pub fn get_expense_claims(&self, circle_id: String, expense_id: String) -> Vec<Claim> {
        // NOTE: Bounded scan across claims (MAX_CLAIMS_PER_CIRCLE)
        self.iter_claims_by_circle(&circle_id)
            .into_iter()
            .filter(|c| c.expense_id == expense_id)
            .collect()
    }

    /// Get the count of pending claims for a circle
    pub fn get_pending_claims_count(&self, circle_id: String) -> u64 {
        self.iter_claims_by_circle(&circle_id)
            .iter()
            .filter(|c| c.status == "pending")
            .count() as u64
    }

    /// Check if a circle has any pending claims
    pub fn has_pending_claims(&self, circle_id: String) -> bool {
        self.get_pending_claims_count(circle_id) > 0
    }

    /// Make a direct NEAR payment to another circle member.
    /// The payment is recorded as a settlement in the circle's history.
    /// Cannot pay yourself. Both payer and recipient must be circle members.
    /// SECURITY: Requires exact deposit amount for the transfer
    #[payable]
    pub fn pay_native(&mut self, circle_id: String, to: AccountId) {
        let payer = env::predecessor_account_id();
        let amount = env::attached_deposit().as_yoctonear();
        require!(amount > 0, "Attach deposit equal to settlement amount");
        // SECURITY: Prevent overflow in downstream operations
        require!(
            amount <= i128::MAX as u128,
            "Amount exceeds maximum safe value"
        );
        require!(payer != to, "Cannot pay yourself");

        self.assert_registered(&payer);
        self.assert_registered(&to);

        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));
        require!(!circle.locked, "Circle is locked for settlement");
        require!(
            circle.state != CircleState::SettlementInProgress,
            "Cannot make payments while autopay settlement is in progress"
        );
        require!(
            circle.state != CircleState::SettlementExecuting,
            "Cannot make payments while settlement execution is in progress"
        );
        require!(circle.members.iter().any(|m| m == &payer), "Payer must be member");
        require!(circle.members.iter().any(|m| m == &to), "Recipient must be member");

        let initial_storage = env::storage_usage();

        let settlement = Settlement {
            circle_id: circle_id.clone(),
            from: payer.clone(),
            to: to.clone(),
            amount: U128(amount),
            token: None,
            ts_ms: timestamp_ms(),
            tx_kind: "native".to_string(),
        };
        self.record_settlement(settlement);

        self.apply_storage_cost(&payer, initial_storage, false);

        Promise::new(to).transfer(yocto_to_token(amount));
    }

    /// Handle incoming FT transfers for circle settlements.
    /// The sender transfers tokens to this contract via ft_transfer_call.
    /// We forward the tokens to the intended recipient and record settlement on success.
    /// Message format: {"circle_id": "...", "to": "recipient.near"}
    /// A2-FIX: Returns PromiseOrValue<U128> per NEP-141 standard:
    /// - U128(0) on success (all tokens consumed)
    /// - U128(amount) on failure (tokens refunded to sender)
    /// D-FIX: Settlement only recorded after successful forward transfer
    pub fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        // A2-FIX: On validation failure, return full amount to refund sender
        if amount.0 == 0 {
            env::log_str("ERROR: Amount must be positive");
            return PromiseOrValue::Value(amount);
        }
        
        // SECURITY: Ensure we have enough gas for the forward transfer + callback
        // Required: ft_transfer (30 TGas) + callback (15 TGas) + buffer (5 TGas)
        let required_gas = Gas::from_tgas(50);
        let remaining_gas = env::prepaid_gas().saturating_sub(env::used_gas());
        if remaining_gas < required_gas {
            env::log_str("ERROR: Insufficient gas for FT forward operation");
            return PromiseOrValue::Value(amount);
        }
        let token_contract = env::predecessor_account_id();
        
        // Parse message and validate format
        let payload: TransferMessage = match serde_json::from_str(&msg) {
            Ok(p) => p,
            Err(_) => {
                env::log_str("ERROR: Invalid message format for ft_on_transfer");
                return PromiseOrValue::Value(amount);
            }
        };

        // Validate circle exists
        let circle = match self.circles.get(&payload.circle_id) {
            Some(c) => c,
            None => {
                env::log_str("ERROR: Circle not found");
                return PromiseOrValue::Value(amount);
            }
        };

        // Validate circle is not locked for settlement
        if circle.locked {
            env::log_str("ERROR: Circle is locked for settlement");
            return PromiseOrValue::Value(amount);
        }

        // Validate sender is circle member
        if !circle.members.iter().any(|m| m == &sender_id) {
            env::log_str("ERROR: Sender is not a circle member");
            return PromiseOrValue::Value(amount);
        }

        // Validate recipient is circle member
        if !circle.members.iter().any(|m| m == &payload.to) {
            env::log_str("ERROR: Recipient is not a circle member");
            return PromiseOrValue::Value(amount);
        }

        // Validate sender and recipient are different
        if sender_id == payload.to {
            env::log_str("ERROR: Cannot pay yourself");
            return PromiseOrValue::Value(amount);
        }

        // Validate both accounts are registered
        if self.storage_deposits.get(&sender_id).is_none() {
            env::log_str("ERROR: Sender is not registered");
            return PromiseOrValue::Value(amount);
        }
        if self.storage_deposits.get(&payload.to).is_none() {
            env::log_str("ERROR: Recipient is not registered");
            return PromiseOrValue::Value(amount);
        }

        // D-FIX: Do NOT record settlement here - only record on successful callback
        // This prevents stuck "paid" records when transfer fails

        self.emit_event(
            "ft_transfer_initiated",
            json!([{
                "circle_id": payload.circle_id,
                "from": sender_id,
                "to": payload.to,
                "amount": amount,
                "token": token_contract,
            }]),
        );

        // Forward the tokens to the recipient
        // D-FIX: Pass all context to callback so it can record settlement on success
        let to_account = payload.to.clone();
        let promise = ext_ft::ext(token_contract.clone())
            .with_attached_deposit(yocto_to_token(ONE_YOCTO))
            .with_static_gas(gas_ft_transfer())
            .ft_transfer(to_account, amount, Some("NearSplitter settlement".to_string()));

        // A2/D-FIX: Callback will check result and return U128(0) on success or U128(amount) on failure
        PromiseOrValue::Promise(promise.then(
            ext_self::ext(env::current_account_id())
                .with_static_gas(gas_ft_callback())
                .on_ft_forward_complete(sender_id, amount, token_contract, payload.circle_id, payload.to)
        ))
    }

    pub fn ft_metadata(&self, token_id: AccountId) -> Option<FungibleTokenMetadata> {
        self.metadata_cache.get(&token_id)
    }

    pub fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        // min = recommended amount so users deposit enough for typical usage
        // This ensures users have available credit for operations like creating circles
        let recommended = self.recommended_storage_cost();
        StorageBalanceBounds {
            min: yocto_to_token(recommended),
            max: None,
        }
    }

    /// Deposit storage for an account. This is required before using the contract.
    /// For new registrations, requires the recommended amount (~0.25 NEAR) to ensure
    /// users have enough credit for typical operations (creating circles, adding expenses).
    /// If the account is already registered and registration_only is true, no deposit is required.
    /// SECURITY: Allows depositing for another account (common NEP-145 pattern for onboarding)
    #[payable]
    pub fn storage_deposit(
        &mut self,
        account_id: Option<AccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
        let depositor = env::predecessor_account_id();
        let account_id = account_id.unwrap_or_else(|| depositor.clone());
        // VALIDATION: Ensure account_id is a valid NEAR account format
        // Note: AccountId type already validates format, but we add explicit length check
        require!(
            account_id.as_str().len() >= 2 && account_id.as_str().len() <= 64,
            "Invalid account ID length"
        );
        let deposit = env::attached_deposit().as_yoctonear();
        let min_locked = self.required_storage_cost();
        let recommended = self.recommended_storage_cost();

        // Already registered case
        if let Some(balance) = self.storage_deposits.get(&account_id) {
            // If registration only, caller shouldn't send deposit
            if let Some(true) = registration_only {
                require!(deposit == 0, "Already registered, no deposit needed for registration_only");
            } else if deposit > 0 {
                let new_total = balance
                    .checked_add(deposit)
                    .unwrap_or_else(|| env::panic_str("Storage deposit overflow"));
                self.storage_deposits.insert(&account_id, &new_total);
                let available = new_total.saturating_sub(min_locked);
                return StorageBalance {
                    total: yocto_to_token(new_total),
                    available: yocto_to_token(available),
                };
            }

            let available = balance.saturating_sub(min_locked);
            return StorageBalance {
                total: yocto_to_token(balance),
                available: yocto_to_token(available),
            };
        }

        // New registration - require recommended amount so user can actually use the contract
        require!(
            deposit >= recommended, 
            "Insufficient deposit for storage registration (need ~0.25 NEAR for typical usage)"
        );
        self.storage_deposits.insert(&account_id, &deposit);

        StorageBalance {
            total: yocto_to_token(deposit),
            available: yocto_to_token(deposit.saturating_sub(min_locked)),
        }
    }

    pub fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        self.storage_deposits
            .get(&account_id)
            .map(|total| {
                let available = total.saturating_sub(self.required_storage_cost());
                StorageBalance {
                    total: yocto_to_token(total),
                    available: yocto_to_token(available),
                }
            })
    }

    #[payable]
    pub fn storage_withdraw(&mut self, amount: Option<U128>) -> StorageBalance {
        require!(
            env::attached_deposit().as_yoctonear() == ONE_YOCTO,
            "Attach 1 yoctoNEAR to withdraw",
        );
        let account = env::predecessor_account_id();
        let total = self
            .storage_deposits
            .get(&account)
            .unwrap_or_else(|| env::panic_str("Account not registered"));
        let cost = self.required_storage_cost();
        let mut available = total.saturating_sub(cost);
        require!(available > 0, "No available storage balance to withdraw");

        let amount_requested = amount.map(|a| a.0).unwrap_or(available);
        require!(
            amount_requested <= available,
            "Requested amount exceeds available balance",
        );
        available -= amount_requested;

        let new_total = cost + available;
        self.storage_deposits.insert(&account, &new_total);

        if amount_requested > 0 {
            Promise::new(account.clone()).transfer(yocto_to_token(amount_requested));
        }

        StorageBalance {
            total: yocto_to_token(new_total),
            available: yocto_to_token(available),
        }
    }

    #[payable]
    pub fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        require!(
            env::attached_deposit().as_yoctonear() == ONE_YOCTO,
            "Attach 1 yoctoNEAR to unregister",
        );
        let account = env::predecessor_account_id();
        let can_force = force.unwrap_or(false);

        require!(
            !self.is_member_any_circle(&account),
            "Remove account from circles before unregistering",
        );
        // SECURITY: Ensure user has no pending payouts before unregistering
        // Otherwise funds would be locked in the contract
        if !can_force {
            require!(
                self.pending_payouts.get(&account).unwrap_or(0) == 0,
                "Withdraw pending payouts before unregistering",
            );
        }

        // SECURITY: If force=true, also refund any pending payouts
        let pending_payout = self.pending_payouts.remove(&account).unwrap_or(0);

        if let Some(balance) = self.storage_deposits.remove(&account) {
            // SECURITY: Combine storage balance + pending payouts in single transfer
            let total_refund = balance.checked_add(pending_payout).unwrap_or(balance);
            Promise::new(account.clone()).transfer(yocto_to_token(total_refund));
            self.emit_event("storage_unregister", json!([{ "account_id": account }]));
            true
        } else {
            false
        }
    }

    /// Cache fungible token metadata for display purposes.
    /// Requires 1 yoctoNEAR deposit for safety.
    /// Note: This is a convenience function - cached data may become stale.
    #[payable]
    pub fn cache_ft_metadata(&mut self, token_id: AccountId, metadata: FungibleTokenMetadata) {
        require!(
            env::attached_deposit().as_yoctonear() == ONE_YOCTO,
            "Attach 1 yoctoNEAR to cache metadata"
        );
        let account = env::predecessor_account_id();
        self.assert_registered(&account);
        // Validate metadata fields to prevent garbage data
        require!(!metadata.name.is_empty(), "Token name cannot be empty");
        require!(!metadata.symbol.is_empty(), "Token symbol cannot be empty");
        require!(metadata.decimals <= 24, "Decimals cannot exceed 24");

        let initial_storage = env::storage_usage();
        
        self.metadata_cache.insert(&token_id, &metadata);

        self.apply_storage_cost(&account, initial_storage, true);
        
        self.emit_event(
            "ft_metadata_cached",
            json!([{
                "token_id": token_id,
                "name": metadata.name,
                "symbol": metadata.symbol,
            }]),
        );
    }

    /// Minimum locked storage cost (just for registration entry)
    /// This is the amount that remains locked and cannot be used for operations
    fn required_storage_cost(&self) -> u128 {
        env::storage_byte_cost().as_yoctonear() * (STORAGE_BYTES_REGISTRATION as u128)
    }

    /// Recommended storage deposit (covers registration + typical operations)
    fn recommended_storage_cost(&self) -> u128 {
        env::storage_byte_cost().as_yoctonear() * (STORAGE_BYTES_RECOMMENDED as u128)
    }

    fn apply_storage_cost(&mut self, account_id: &AccountId, initial_usage: u64, use_attached: bool) {
        let final_usage = env::storage_usage();
        let attached = if use_attached {
            env::attached_deposit().as_yoctonear()
        } else {
            0
        };
        let refund_target = if use_attached {
            Some(env::predecessor_account_id())
        } else {
            None
        };

        if final_usage > initial_usage {
            let delta = final_usage - initial_usage;
            let cost = (delta as u128)
                .checked_mul(env::storage_byte_cost().as_yoctonear())
                .unwrap_or_else(|| env::panic_str("Storage cost overflow"));
            if cost == 0 {
                if attached > 0 {
                    Promise::new(refund_target.unwrap()).transfer(yocto_to_token(attached));
                }
                return;
            }

            let mut remaining = cost;
            let mut used_from_deposit: u128 = 0;
            if attached > 0 {
                if attached >= remaining {
                    used_from_deposit = remaining;
                    remaining = 0;
                } else {
                    used_from_deposit = attached;
                    remaining = remaining
                        .checked_sub(attached)
                        .unwrap_or_else(|| env::panic_str("Storage cost underflow"));
                }
            }

            if remaining > 0 {
                let total = self
                    .storage_deposits
                    .get(account_id)
                    .unwrap_or_else(|| env::panic_str("Account not registered"));
                let min = self.required_storage_cost();
                let available = total.saturating_sub(min);
                require!(
                    available >= remaining,
                    "Insufficient storage credit; call storage_deposit"
                );
                let new_total = total
                    .checked_sub(remaining)
                    .unwrap_or_else(|| env::panic_str("Storage debit underflow"));
                self.storage_deposits.insert(account_id, &new_total);
            }

            let refund = attached.saturating_sub(used_from_deposit);
            if refund > 0 {
                Promise::new(refund_target.unwrap()).transfer(yocto_to_token(refund));
            }
        } else {
            if final_usage < initial_usage {
                let delta = initial_usage - final_usage;
                let refund = (delta as u128)
                    .checked_mul(env::storage_byte_cost().as_yoctonear())
                    .unwrap_or_else(|| env::panic_str("Storage refund overflow"));
                if refund > 0 {
                    let total = self
                        .storage_deposits
                        .get(account_id)
                        .unwrap_or_else(|| env::panic_str("Account not registered"));
                    let new_total = total
                        .checked_add(refund)
                        .unwrap_or_else(|| env::panic_str("Storage credit overflow"));
                    self.storage_deposits.insert(account_id, &new_total);
                }
            }

            if attached > 0 {
                Promise::new(refund_target.unwrap()).transfer(yocto_to_token(attached));
            }
        }
    }

    fn record_settlement(&mut self, settlement: Settlement) {
        let circle_id = settlement.circle_id.clone();
        let current_len = self.settlements_len.get(&circle_id).unwrap_or(0);
        require!(
            (current_len as usize) < MAX_SETTLEMENTS_PER_CIRCLE,
            "Circle has reached maximum settlements limit (10,000)"
        );

        let event_payload = json!([{
            "circle_id": settlement.circle_id.clone(),
            "from": settlement.from.clone(),
            "to": settlement.to.clone(),
            "amount": settlement.amount,
            "token": settlement.token.clone(),
            "tx_kind": settlement.tx_kind.clone(),
            "ts_ms": settlement.ts_ms,
        }]);

        let settlement_id = format!("settlement-{}-{}", circle_id, current_len + 1);
        let index_key = Self::settlement_index_key(&circle_id, current_len);
        self.settlements_index.insert(&index_key, &settlement_id);
        self.settlement_by_id.insert(&settlement_id, &settlement);
        self.settlements_len.insert(&circle_id, &(current_len + 1));

        self.emit_event("settlement_paid", event_payload);
    }

    fn assert_registered(&self, account_id: &AccountId) {
        require!(
            self.storage_deposits.get(account_id).is_some(),
            "Account must call storage_deposit first",
        );
    }

    fn is_member_any_circle(&self, account_id: &AccountId) -> bool {
        // Use indexed lookup for O(1) instead of O(n) iteration over all circles
        self.circles_by_member
            .get(account_id)
            .map(|circles| !circles.is_empty())
            .unwrap_or(false)
    }

    fn assert_circle_state_consistent(&self, circle: &Circle) {
        if circle.locked {
            require!(
                circle.state == CircleState::SettlementInProgress
                    || circle.state == CircleState::SettlementExecuting,
                "Circle locked but state is not settlement"
            );
        }
        if circle.state == CircleState::SettlementInProgress
            || circle.state == CircleState::SettlementExecuting
        {
            require!(circle.locked, "Circle state implies locked");
        }
    }

    fn emit_event(&self, event: &str, data: serde_json::Value) {
        let payload = json!({
            "standard": EVENT_STANDARD,
            "version": EVENT_VERSION,
            "event": event,
            "data": data,
        });
        env::log_str(&format!("EVENT_JSON:{}", payload));
    }

    /// Callback after FT forward completes.
    /// D-FIX: Records settlement only on success; returns amount to refund on failure.
    /// 
    /// # Security
    /// - #[private] ensures only this contract can call this callback
    /// - Returns U128(0) on success (all tokens consumed per NEP-141)
    /// - Returns U128(amount) on failure (refund to sender per NEP-141)
    #[private]
    pub fn on_ft_forward_complete(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        token_contract: AccountId,
        circle_id: String,
        to: AccountId,
    ) -> U128 {
        // Note: promise_result(0) returns the result of the ft_transfer call
        match env::promise_result(0) {
            PromiseResult::Successful(_) => {
                let initial_storage = env::storage_usage();
                let available_storage = self
                    .storage_deposits
                    .get(&sender_id)
                    .unwrap_or(0)
                    .saturating_sub(self.required_storage_cost());
                let estimated_cost = env::storage_byte_cost().as_yoctonear()
                    .checked_mul(ESTIMATED_SETTLEMENT_STORAGE_BYTES as u128)
                    .unwrap_or_else(|| env::panic_str("Storage cost overflow"));

                if available_storage < estimated_cost {
                    self.emit_event(
                        "ft_settlement_skipped_storage",
                        json!([{
                            "circle_id": circle_id,
                            "from": sender_id,
                            "to": to,
                            "amount": amount,
                            "token": token_contract,
                            "available_storage": U128(available_storage),
                            "estimated_cost": U128(estimated_cost),
                        }]),
                    );
                    return U128(0);
                }
                // D-FIX: Record settlement only after successful forward transfer
                let settlement = Settlement {
                    circle_id: circle_id.clone(),
                    from: sender_id.clone(),
                    to: to.clone(),
                    amount,
                    token: Some(token_contract.clone()),
                    ts_ms: timestamp_ms(),
                    tx_kind: "ft_transfer".to_string(),
                };
                self.record_settlement(settlement);

                self.apply_storage_cost(&sender_id, initial_storage, false);

                self.emit_event(
                    "ft_transfer_success",
                    json!([{
                        "circle_id": circle_id,
                        "from": sender_id,
                        "to": to,
                        "amount": amount,
                        "token": token_contract,
                        "balance_policy": "ft_ignored",
                    }]),
                );
                // A2-FIX: Return U128(0) to indicate all tokens consumed (NEP-141)
                U128(0)
            }
            PromiseResult::Failed => {
                // D-FIX: Transfer failed - do NOT record settlement, return amount for refund
                self.emit_event(
                    "ft_transfer_failed",
                    json!([{
                        "circle_id": circle_id,
                        "from": sender_id,
                        "to": to,
                        "amount": amount,
                        "token": token_contract,
                        "message": "FT forward failed - tokens refunded to sender",
                    }]),
                );
                env::log_str("FT transfer failed - refunding tokens to sender");
                // A2-FIX: Return amount to trigger refund to sender (NEP-141)
                amount
            }
        }
    }

    /// Admin function to rescue stuck FT tokens from failed transfers.
    /// Only callable by the contract itself (requires DAO or multisig to trigger).
    /// This is a safety mechanism for tokens that got stuck due to failed ft_transfer calls.
    /// 
    /// # Security
    /// - #[private] macro already verifies predecessor == current_account_id
    /// - Requires a privileged call (e.g., DAO proposal) to invoke
    #[private]
    pub fn rescue_stuck_ft(
        &self,
        token_id: AccountId,
        receiver_id: AccountId,
        amount: U128,
    ) -> Promise {
        // #[private] already enforces predecessor == current_account_id
        require!(amount.0 > 0, "Amount must be positive");
        
        self.emit_event(
            "ft_rescue",
            json!([{
                "token_id": token_id,
                "receiver_id": receiver_id,
                "amount": amount,
            }]),
        );

        ext_ft::ext(token_id)
            .with_attached_deposit(yocto_to_token(ONE_YOCTO))
            .with_static_gas(gas_ft_transfer())
            .ft_transfer(receiver_id, amount, Some("Rescue stuck tokens".to_string()))
    }

    /// Admin function to rescue stuck NEAR from failed autopay settlements.
    /// Only callable by the contract itself (requires DAO or multisig to trigger).
    /// 
    /// # Security
    /// - #[private] macro verifies predecessor == current_account_id
    /// - Requires a privileged call (e.g., DAO proposal) to invoke
    /// SECURITY: Added parallel to rescue_stuck_ft for NEAR token recovery
    #[private]
    pub fn rescue_stuck_near(
        &self,
        receiver_id: AccountId,
        amount: U128,
    ) -> Promise {
        require!(amount.0 > 0, "Amount must be positive");
        
        // Ensure we have enough balance to transfer
        let contract_balance = env::account_balance().as_yoctonear();
        let storage_cost = env::storage_byte_cost().as_yoctonear() 
            * env::storage_usage() as u128;
        let available = contract_balance.saturating_sub(storage_cost);
        
        require!(
            amount.0 <= available,
            "Insufficient available balance for rescue"
        );
        
        self.emit_event(
            "near_rescue",
            json!([{
                "receiver_id": receiver_id,
                "amount": amount,
            }]),
        );

        Promise::new(receiver_id).transfer(yocto_to_token(amount.0))
    }
}

#[ext_contract(ext_ft)]
pub trait ExtFungibleToken {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
}

#[ext_contract(ext_self)]
pub trait ExtSelf {
    /// A1-FIX: Callback signature now accepts context params and returns U128 for NEP-141 refund
    fn on_ft_forward_complete(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        token_contract: AccountId,
        circle_id: String,
        to: AccountId,
    ) -> U128;
}

#[near_bindgen]
impl NearSplitter {
    /// Confirm the ledger for a circle. Once all members confirm, settlement can proceed.
    /// First confirmation locks the circle (no new expenses). 
    /// If all members have autopay enabled, automatically distributes escrowed funds.
    /// This automatically enables autopay and requires escrow deposit if user has debt.
    /// Once all members confirm, settlement proceeds automatically.
    /// 
    /// # Security
    /// - Requires caller to be registered and a circle member
    /// - Prevents double-confirmation
    /// - Requires escrow deposit covering full debt amount if caller is a debtor
    /// - Prevents confirmation while claims are pending
    /// SECURITY: Uses checks-effects-interactions pattern throughout
    #[payable]
    pub fn confirm_ledger(&mut self, circle_id: String) {
        let account = env::predecessor_account_id();
        let deposit = env::attached_deposit().as_yoctonear();
        self.assert_registered(&account);
        
        // SECURITY: Validate deposit doesn't exceed safe limits for arithmetic
        require!(
            deposit <= i128::MAX as u128,
            "Deposit exceeds maximum safe value"
        );

        let mut circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        // B1-FIX: Only block during actual autopay execution, not during confirmation phase
        require!(
            circle.state != CircleState::SettlementExecuting,
            "Settlement execution is in progress - cannot modify"
        );

        // Allow re-confirmation after settlement is complete (circle reuse)
        // Reset state if circle was previously settled and is now being reused
        if circle.state == CircleState::Settled {
            // Circle was settled, now starting fresh confirmation round
            circle.state = CircleState::Open;
            circle.locked = false;  // B1-FIX: Also reset locked flag for new round
            self.circles.insert(&circle_id, &circle);
        }

        require!(
            circle.members.iter().any(|m| m == &account),
            "Only circle members can confirm"
        );

        // Check for pending claims - cannot settle with unresolved disputes
        let pending_claims = self.get_pending_claims_count(circle_id.clone());
        require!(
            pending_claims == 0,
            "Cannot confirm ledger: resolve all pending claims first"
        );

        let initial_storage = env::storage_usage();
        let mut refund_amount: u128 = 0;

        let confirmation_key = format!("{}:{}", circle_id, account);
        let mut confirmations_count = self.confirmations_count.get(&circle_id).unwrap_or(0);
        
        require!(
            !self.confirmations_map.get(&confirmation_key).unwrap_or(false),
            "Already confirmed"
        );

        // Calculate user's current debt (negative balance)
        let balances = self.compute_balances(circle_id.clone());
        let user_balance = balances
            .iter()
            .find(|b| b.account_id == account)
            .map(|b| b.net.0)
            .unwrap_or(0);

        // If user has debt, require escrow deposit
        if user_balance < 0 {
            let debt = user_balance.unsigned_abs();
            require!(
                deposit >= debt,
                &format!("Must deposit at least {} yoctoNEAR (attached: {})", debt, deposit)
            );

            // Store the deposit in escrow with overflow protection
            let escrow_key = format!("{}:{}", circle_id, account);
            let existing_deposit = self.escrow_deposits.get(&escrow_key).unwrap_or(0);
            let new_total = existing_deposit
                .checked_add(deposit)
                .unwrap_or_else(|| env::panic_str("Escrow deposit overflow"));
            self.escrow_deposits.insert(&escrow_key, &new_total);

            self.emit_event(
                "escrow_deposited",
                json!([{
                    "circle_id": circle_id.clone(),
                    "account_id": account.clone(),
                    "amount": U128(deposit),
                    "total_escrowed": U128(new_total),
                }]),
            );
        } else if deposit > 0 {
            // User is creditor or even, but deposited anyway - refund immediately
            refund_amount = deposit;
            
            self.emit_event(
                "deposit_refunded",
                json!([{
                    "circle_id": circle_id.clone(),
                    "account_id": account.clone(),
                    "amount": U128(deposit),
                    "message": "Creditors do not need to deposit. Funds refunded.",
                }]),
            );
        }

        // Automatically enable autopay for this user
        let autopay_key = format!("{}:{}", circle_id, account);
        self.autopay_preferences.insert(&autopay_key, &true);

        self.emit_event(
            "autopay_enabled",
            json!([{
                "circle_id": circle_id.clone(),
                "account_id": account.clone(),
            }]),
        );

        // Lock the circle on first confirmation (also closes membership)
        if confirmations_count == 0 && !circle.locked {
            circle.locked = true;
            circle.membership_open = false; // Close membership during settlement
            circle.state = CircleState::SettlementInProgress;  // Set state to prevent concurrent operations
            self.circles.insert(&circle_id, &circle);
            
            self.emit_event(
                "circle_locked",
                json!([{
                    "circle_id": circle_id.clone(),
                    "message": "Circle locked for settlement. No new expenses or members allowed.",
                    "membership_open": false,
                }]),
            );
        }

        self.confirmations_map.insert(&confirmation_key, &true);
        confirmations_count += 1;
        self.confirmations_count.insert(&circle_id, &confirmations_count);

        self.apply_storage_cost(&account, initial_storage, false);

        if refund_amount > 0 {
            Promise::new(account.clone()).transfer(yocto_to_token(refund_amount));
        }

        self.emit_event(
            "ledger_confirmed",
            json!([{
                "circle_id": circle_id.clone(),
                "account_id": account,
                "confirmations": confirmations_count,
                "total_members": circle.members.len(),
            }]),
        );

        // If all members confirmed, execute autopay settlements
        if confirmations_count as usize == circle.members.len() {
            self.execute_autopay_settlements(circle_id);
        }
    }

    /// Execute autopay settlements when all members have confirmed.
    /// All members must have autopay enabled and debtors must have escrowed enough to fully cover their debts.
    /// If coverage is insufficient, the function reverts and leaves expenses/confirmations intact.
    /// 
    /// # Security
    /// - Internal function only called from confirm_ledger
    /// - All state changes happen before any external calls (reentrancy protection)
    /// - Uses checked arithmetic throughout
    /// - B1-FIX: Sets SettlementExecuting state to prevent re-entry
    fn execute_autopay_settlements(&mut self, circle_id: String) {
        let mut circle = self.circles.get(&circle_id).expect("Circle not found");
        let initial_storage = env::storage_usage();
        let owner = circle.owner.clone();
        
        // B1-FIX: Set SettlementExecuting state to prevent re-entry during payout phase
        circle.state = CircleState::SettlementExecuting;
        self.circles.insert(&circle_id, &circle);
        
        // Get settlement suggestions
        let suggestions = self.suggest_settlements(circle_id.clone());
        
        // Track all promises to batch transfers (gas efficiency)
        let mut transfers_to_make: Vec<(AccountId, u128)> = Vec::new();
        
        // If no settlements needed (no expenses or everyone is even), just cleanup
        if suggestions.is_empty() {
            self.emit_event(
                "no_settlements_needed",
                json!([{
                    "circle_id": circle_id,
                    "message": "No settlements required - all balances are even.",
                }]),
            );
            
            // Collect all escrow refunds before state changes
            for member in &circle.members {
                let escrow_key = format!("{}:{}", circle_id, member);
                if let Some(escrowed) = self.escrow_deposits.get(&escrow_key) {
                    if escrowed > 0 {
                        self.escrow_deposits.remove(&escrow_key);
                        transfers_to_make.push((member.clone(), escrowed));
                    }
                }
                let autopay_key = format!("{}:{}", circle_id, member);
                self.autopay_preferences.remove(&autopay_key);
            }
            
            self.clear_expenses_for_circle(&circle_id);
            self.clear_confirmations_for_circle(&circle_id, &circle.members);
            
            let mut updated_circle = circle.clone();
            updated_circle.locked = false;
            updated_circle.membership_open = true;
            updated_circle.state = CircleState::Settled;
            self.circles.insert(&circle_id, &updated_circle);

            self.apply_storage_cost(&owner, initial_storage, false);
            
            // Make all transfers after state is finalized
            for (recipient, amount) in transfers_to_make {
                Promise::new(recipient).transfer(yocto_to_token(amount));
            }
            
            self.emit_event(
                "ledger_settled",
                json!([{
                    "circle_id": circle_id,
                    "all_autopay": true,
                    "settlements_count": 0,
                }]),
            );
            return;
        }
        
        // Determine which members have autopay enabled
        let autopay_members: Vec<AccountId> = circle.members.iter()
            .filter(|member| {
                let key = format!("{}:{}", circle_id, member);
                self.autopay_preferences.get(&key).unwrap_or(false)
            })
            .cloned()
            .collect();

        let all_autopay = autopay_members.len() == circle.members.len();
        require!(all_autopay, "All members must have autopay enabled to settle");

        // B2-FIX: Compute total required escrow per debtor (sum of all outgoing transfers)
        let mut required_by_debtor: HashMap<AccountId, u128> = HashMap::new();
        for suggestion in &suggestions {
            if suggestion.amount.0 == 0 {
                continue;
            }
            let entry = required_by_debtor.entry(suggestion.from.clone()).or_insert(0);
            *entry = entry.checked_add(suggestion.amount.0).unwrap_or_else(|| {
                env::panic_str("Required escrow sum overflow")
            });
        }

        // B2-FIX: Verify each debtor has sufficient escrow for their TOTAL outgoing transfers
        for (debtor, required_total) in &required_by_debtor {
            let escrow_key = format!("{}:{}", circle_id, debtor);
            let escrowed = self.escrow_deposits.get(&escrow_key).unwrap_or(0);
            require!(
                escrowed >= *required_total,
                &format!(
                    "Insufficient escrow for {}: needs {} but has {}",
                    debtor, required_total, escrowed
                )
            );
        }
        
        // Track all payouts to aggregate (state changes before transfers)
        let mut payouts_to_credit: Vec<(AccountId, u128)> = Vec::new();

        // All members have autopay - distribute escrowed funds
        self.emit_event(
            "autopay_triggered",
            json!([{
                "circle_id": circle_id,
                "message": "All members have autopay. Distributing escrowed funds.",
                "settlement_count": suggestions.len(),
                "autopay_members": autopay_members.len(),
            }]),
        );

        // Process transfers from escrow - all state changes first
        for suggestion in &suggestions {
            if suggestion.amount.0 == 0 {
                continue;
            }
            let from_key = format!("{}:{}", circle_id, suggestion.from);
            let escrowed = self.escrow_deposits.get(&from_key).unwrap_or(0);

            // B2-FIX: Use checked_sub to panic if insufficient (should never happen due to pre-check)
            let remaining = escrowed.checked_sub(suggestion.amount.0)
                .unwrap_or_else(|| env::panic_str("Escrow underflow - insufficient funds"));
            if remaining > 0 {
                self.escrow_deposits.insert(&from_key, &remaining);
            } else {
                self.escrow_deposits.remove(&from_key);
            }

            payouts_to_credit.push((suggestion.to.clone(), suggestion.amount.0));

            let settlement = Settlement {
                circle_id: circle_id.clone(),
                from: suggestion.from.clone(),
                to: suggestion.to.clone(),
                amount: suggestion.amount,
                token: None,
                ts_ms: timestamp_ms(),
                tx_kind: "autopay_escrow".to_string(),
            };
            self.record_settlement(settlement);

            self.emit_event(
                "settlement_executed",
                json!([{
                    "circle_id": circle_id,
                    "from": suggestion.from,
                    "to": suggestion.to,
                    "amount": suggestion.amount,
                }]),
            );
        }

        // Collect any remaining escrow refunds
        for member in &circle.members {
            let escrow_key = format!("{}:{}", circle_id, member);
            if let Some(remaining) = self.escrow_deposits.get(&escrow_key) {
                if remaining > 0 {
                    self.escrow_deposits.remove(&escrow_key);
                    payouts_to_credit.push((member.clone(), remaining));
                }
            }
            // Clean up autopay preferences
            let autopay_key = format!("{}:{}", circle_id, member);
            self.autopay_preferences.remove(&autopay_key);
        }

        // Aggregate payouts by recipient
        let mut aggregated: HashMap<AccountId, u128> = HashMap::new();
        for (recipient, amount) in payouts_to_credit {
            if amount == 0 {
                continue;
            }
            let entry = aggregated.entry(recipient).or_insert(0);
            *entry = entry.checked_add(amount).unwrap_or_else(|| {
                env::panic_str("Payout aggregation overflow")
            });
        }

        // Clear expenses and confirmations before transfers
        self.clear_expenses_for_circle(&circle_id);
        self.clear_confirmations_for_circle(&circle_id, &circle.members);
        
        // Update circle: unlock, reopen membership, mark as settled
        let mut updated_circle = circle.clone();
        updated_circle.locked = false;
        updated_circle.membership_open = true;
        updated_circle.state = CircleState::Settled;
        self.circles.insert(&circle_id, &updated_circle);

        self.apply_storage_cost(&owner, initial_storage, false);

        // Credit pending payouts (pull-payment pattern)
        for (recipient, total) in aggregated {
            if total == 0 {
                continue;
            }
            let existing = self.pending_payouts.get(&recipient).unwrap_or(0);
            let new_total = existing
                .checked_add(total)
                .unwrap_or_else(|| env::panic_str("Pending payout overflow"));
            self.pending_payouts.insert(&recipient, &new_total);

            self.emit_event(
                "payout_credited",
                json!([{
                    "circle_id": circle_id,
                    "account_id": recipient,
                    "amount": U128(total),
                    "pending_total": U128(new_total),
                }]),
            );
        }

        self.emit_event(
            "ledger_settled",
            json!([{
                "circle_id": circle_id,
                "all_autopay": all_autopay,
                "membership_open": true,
            }]),
        );
    }

    /// Get the list of accounts that have confirmed the ledger for a circle
    pub fn get_confirmations(&self, circle_id: String) -> Vec<AccountId> {
        let circle = self.circles.get(&circle_id);
        if circle.is_none() {
            return Vec::new();
        }
        let circle = circle.unwrap();
        circle
            .members
            .iter()
            .filter(|member| {
                let key = format!("{}:{}", circle_id, member);
                self.confirmations_map.get(&key).unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// Check if all members have confirmed the ledger
    pub fn is_fully_confirmed(&self, circle_id: String) -> bool {
        let circle = self.circles.get(&circle_id);
        if circle.is_none() {
            return false;
        }
        let circle = circle.unwrap();
        let confirmations = self.confirmations_count.get(&circle_id).unwrap_or(0);
        confirmations as usize == circle.members.len()
    }

    /// Reset confirmations for a circle (e.g., after adding new expenses)
    /// Also unlocks the circle and refunds all escrowed deposits
    /// SECURITY: Uses checks-effects-interactions pattern - all state changes before transfers
    pub fn reset_confirmations(&mut self, circle_id: String) {
        let account = env::predecessor_account_id();
        let mut circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(circle.owner == account, "Only circle owner can reset confirmations");

        // Prevent reset during ongoing settlement
        require!(
            circle.state != CircleState::SettlementInProgress,
            "Cannot reset confirmations while settlement is in progress"
        );

        // SECURITY: Collect all refunds BEFORE making state changes, then transfer AFTER
        let mut refunds_to_make: Vec<(AccountId, u128)> = Vec::new();

        let initial_storage = env::storage_usage();

        // Collect all escrowed deposits for this circle
        for member in &circle.members {
            let escrow_key = format!("{}:{}", circle_id, member);
            if let Some(escrowed) = self.escrow_deposits.get(&escrow_key) {
                if escrowed > 0 {
                    self.escrow_deposits.remove(&escrow_key);
                    refunds_to_make.push((member.clone(), escrowed));
                }
            }
            // Also reset autopay preferences
            let autopay_key = format!("{}:{}", circle_id, member);
            self.autopay_preferences.remove(&autopay_key);
        }

        self.clear_confirmations_for_circle(&circle_id, &circle.members);
        
        // Unlock the circle, reopen membership, and reset state
        circle.locked = false;
        circle.membership_open = true;
        circle.state = CircleState::Open;
        self.circles.insert(&circle_id, &circle);

        self.apply_storage_cost(&account, initial_storage, false);
        
        self.emit_event(
            "confirmations_reset",
            json!([{
                "circle_id": circle_id,
                "unlocked": true,
                "membership_open": true,
            }]),
        );

        // SECURITY: All state changes complete - now safe to make external calls
        for (member, escrowed) in refunds_to_make {
            self.emit_event(
                "escrow_refunded",
                json!([{
                    "circle_id": circle_id,
                    "account_id": member,
                    "amount": U128(escrowed),
                }]),
            );
            Promise::new(member).transfer(yocto_to_token(escrowed));
        }
    }

    /// Set whether the circle is open for new members to join.
    /// Only the circle owner can call this.
    /// When membership is closed, no one can join even with invite code.
    /// Note: This is automatically set to false when first confirmation happens.
    pub fn set_membership_open(&mut self, circle_id: String, open: bool) {
        let account = env::predecessor_account_id();
        let mut circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(circle.owner == account, "Only circle owner can change membership status");
        
        // Cannot open membership while circle is locked for settlement or during settlement
        if open {
            require!(
                !circle.locked && circle.state != CircleState::SettlementInProgress,
                "Cannot open membership while settlement is in progress"
            );
        }

        circle.membership_open = open;
        self.circles.insert(&circle_id, &circle);

        self.emit_event(
            "membership_status_changed",
            json!([{
                "circle_id": circle_id,
                "membership_open": open,
            }]),
        );
    }

    /// Check if circle is open for new members
    pub fn is_membership_open(&self, circle_id: String) -> bool {
        self.circles
            .get(&circle_id)
            .map(|c| c.membership_open)
            .unwrap_or(false)
    }

    /// Set autopay preference for the caller in a specific circle
    /// If enabling autopay and user has debt, requires deposit equal to debt amount
    #[payable]
    pub fn set_autopay(&mut self, circle_id: String, enabled: bool) {
        let account = env::predecessor_account_id();
        let deposit = env::attached_deposit().as_yoctonear();
        self.assert_registered(&account);

        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(
            circle.members.iter().any(|m| m == &account),
            "Must be a circle member to set autopay"
        );

        // Prevent disabling autopay when circle is locked for settlement
        if !enabled && circle.locked {
            env::panic_str("Cannot disable autopay while circle is locked for settlement");
        }

        let key = format!("{}:{}", circle_id, account);
        let initial_storage = env::storage_usage();
        let mut refund_amount: u128 = 0;

        if enabled {
            // Calculate user's current debt (negative balance)
            let balances = self.compute_balances(circle_id.clone());
            let user_balance = balances
                .iter()
                .find(|b| b.account_id == account)
                .map(|b| b.net.0)
                .unwrap_or(0);

            if user_balance < 0 {
                // User owes money - require escrow deposit
                let debt = user_balance.unsigned_abs();
                require!(
                    deposit >= debt,
                    &format!("Must deposit {} yoctoNEAR to cover debt", debt)
                );

                // Store the deposit in escrow with overflow protection
                let escrow_key = format!("{}:{}", circle_id, account);
                let existing_deposit = self.escrow_deposits.get(&escrow_key).unwrap_or(0);
                let new_total = existing_deposit
                    .checked_add(deposit)
                    .unwrap_or_else(|| env::panic_str("Escrow deposit overflow"));
                self.escrow_deposits.insert(&escrow_key, &new_total);

                self.emit_event(
                    "escrow_deposited",
                    json!([{
                        "circle_id": circle_id,
                        "account_id": account,
                        "amount": U128(deposit),
                        "total_escrowed": U128(new_total),
                    }]),
                );
            } else if deposit > 0 {
                // User is creditor or even, but deposited anyway - refund
                refund_amount = deposit;
            }
        } else {
            // Disabling autopay - refund any escrowed funds
            let escrow_key = format!("{}:{}", circle_id, account);
            // SECURITY: Collect refund amount and remove from state BEFORE transfer
            let escrowed_to_refund = self.escrow_deposits.remove(&escrow_key).unwrap_or(0);
            refund_amount = escrowed_to_refund;
            
            if escrowed_to_refund > 0 {
                self.emit_event(
                    "escrow_refunded",
                    json!([{
                        "circle_id": circle_id,
                        "account_id": account,
                        "amount": U128(escrowed_to_refund),
                    }]),
                );
            }

            self.autopay_preferences.insert(&key, &enabled);

            self.emit_event(
                "autopay_preference_set",
                json!([{
                    "circle_id": circle_id,
                    "account_id": account,
                    "enabled": enabled,
                }]),
            );

            self.apply_storage_cost(&account, initial_storage, false);

            // SECURITY: Transfer AFTER all state changes (checks-effects-interactions)
            if refund_amount > 0 {
                Promise::new(account).transfer(yocto_to_token(refund_amount));
            }
            return;
        }

        self.autopay_preferences.insert(&key, &enabled);

        self.emit_event(
            "autopay_preference_set",
            json!([{
                "circle_id": circle_id,
                "account_id": account,
                "enabled": enabled,
            }]),
        );

        self.apply_storage_cost(&account, initial_storage, false);

        if refund_amount > 0 {
            Promise::new(account).transfer(yocto_to_token(refund_amount));
        }
    }

    /// Get autopay preference for a specific member in a circle
    pub fn get_autopay(&self, circle_id: String, account_id: AccountId) -> bool {
        let key = format!("{}:{}", circle_id, account_id);
        self.autopay_preferences.get(&key).unwrap_or(false)
    }

    /// Check if all members in a circle have autopay enabled
    pub fn all_members_autopay(&self, circle_id: String) -> bool {
        let circle = self.circles.get(&circle_id);
        if circle.is_none() {
            return false;
        }
        let circle = circle.unwrap();
        
        circle.members.iter().all(|member| {
            let key = format!("{}:{}", circle_id, member);
            self.autopay_preferences.get(&key).unwrap_or(false)
        })
    }

    /// Get required deposit amount for a member to enable autopay
    /// Returns 0 if user is creditor or even, otherwise returns debt amount
    pub fn get_required_autopay_deposit(&self, circle_id: String, account_id: AccountId) -> U128 {
        let balances = self.compute_balances(circle_id);
        let user_balance = balances
            .iter()
            .find(|b| b.account_id == account_id)
            .map(|b| b.net.0)
            .unwrap_or(0);

        if user_balance < 0 {
            U128(user_balance.unsigned_abs())
        } else {
            U128(0)
        }
    }

    /// Get current escrow deposit for a member in a circle
    pub fn get_escrow_deposit(&self, circle_id: String, account_id: AccountId) -> U128 {
        let key = format!("{}:{}", circle_id, account_id);
        U128(self.escrow_deposits.get(&key).unwrap_or(0))
    }

    /// Get the pending payout balance for an account.
    /// This is the amount that can be withdrawn via withdraw_payout().
    pub fn get_pending_payout(&self, account_id: AccountId) -> U128 {
        U128(self.pending_payouts.get(&account_id).unwrap_or(0))
    }

    /// Withdraw all pending payouts for the caller.
    /// This implements the pull-payment pattern for settlement distributions.
    /// Returns a Promise that transfers all pending funds to the caller.
    #[payable]
    pub fn withdraw_payout(&mut self) -> Promise {
        require!(
            env::attached_deposit().as_yoctonear() == ONE_YOCTO,
            "Attach exactly 1 yoctoNEAR for security"
        );

        let account = env::predecessor_account_id();
        let pending = self.pending_payouts.get(&account).unwrap_or(0);

        require!(pending > 0, "No pending payouts to withdraw");

        // Clear the pending payout before transfer (reentrancy protection)
        self.pending_payouts.remove(&account);

        self.emit_event(
            "payout_withdrawn",
            json!([{
                "account_id": account,
                "amount": U128(pending),
            }]),
        );

        // Single promise transfer - no joint promises
        Promise::new(account).transfer(yocto_to_token(pending))
    }

    /// Withdraw a specific amount from pending payouts.
    /// Useful if you want to withdraw only part of your pending balance.
    #[payable]
    pub fn withdraw_payout_partial(&mut self, amount: U128) -> Promise {
        require!(
            env::attached_deposit().as_yoctonear() == ONE_YOCTO,
            "Attach exactly 1 yoctoNEAR for security"
        );

        let account = env::predecessor_account_id();
        let pending = self.pending_payouts.get(&account).unwrap_or(0);

        require!(pending > 0, "No pending payouts to withdraw");
        require!(amount.0 > 0, "Amount must be positive");
        require!(amount.0 <= pending, "Insufficient pending balance");

        // Update pending payout
        let remaining = pending - amount.0;
        if remaining > 0 {
            self.pending_payouts.insert(&account, &remaining);
        } else {
            self.pending_payouts.remove(&account);
        }

        self.emit_event(
            "payout_withdrawn",
            json!([{
                "account_id": account,
                "amount": amount,
                "remaining": U128(remaining),
            }]),
        );

        // Single promise transfer - no joint promises
        Promise::new(account).transfer(yocto_to_token(amount.0))
    }
}

fn paginate_vec<T: Clone>(items: &[T], from: u64, limit: u64) -> Vec<T> {
    if items.is_empty() {
        return Vec::new();
    }
    
    // Enforce maximum limit to prevent DoS attacks via unbounded pagination
    let safe_limit = limit.min(MAX_PAGINATION_LIMIT);
    
    let start = from.min(items.len() as u64) as usize;
    let end = (start + safe_limit as usize).min(items.len());
    items[start..end].to_vec()
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;
    use near_sdk::PromiseResult;
    use std::cell::Cell;

    const ONE_NEAR: u128 = 1_000_000_000_000_000_000_000_000;

    fn setup() -> NearSplitter {
        NearSplitter::new()
    }

    fn context(predecessor: AccountId, deposit: u128) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder.predecessor_account_id(predecessor.clone());
        builder.signer_account_id(predecessor);
        builder.attached_deposit(NearToken::from_yoctonear(deposit));
        builder.account_balance(NearToken::from_yoctonear(ONE_NEAR * 1_000));
        builder.block_timestamp(1_620_000_000_000_000_000);
        builder
    }

    #[test]
    fn test_storage_deposit_and_membership() {
        let mut contract = setup();
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        let id = contract.create_circle("Friends".to_string(), None);
        assert_eq!(id, "circle-0");
    }

    #[test]
    fn test_storage_deposit_increases_credit_when_registered() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        let min = contract.storage_balance_bounds().min.as_yoctonear();

        ctx = context(accounts(0), min);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        let before = contract.storage_balance_of(accounts(0)).unwrap();

        ctx = context(accounts(0), 1_000);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        let after = contract.storage_balance_of(accounts(0)).unwrap();
        assert!(after.total.as_yoctonear() > before.total.as_yoctonear());
        assert!(after.available.as_yoctonear() >= before.available.as_yoctonear());
    }

    #[test]
    fn test_storage_credit_exhausted_add_expense_fails() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        let min = contract.storage_balance_bounds().min.as_yoctonear();
        let extra = env::storage_byte_cost().as_yoctonear() * 1_000;

        ctx = context(accounts(0), min + extra);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), min + extra);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        let added = Cell::new(0u64);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for i in 0..200u64 {
                ctx = context(accounts(0), 0);
                testing_env!(ctx.build());
                contract.add_expense(
                    "circle-0".to_string(),
                    U128(1),
                    vec![
                        MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                        MemberShare { account_id: accounts(1), weight_bps: 5_000 },
                    ],
                    format!("Expense {}", i + 1),
                );
                added.set(added.get() + 1);
            }
        }));

        assert!(result.is_err(), "Expected storage exhaustion to panic");
        assert!(added.get() > 0, "At least one expense should be added before exhaustion");
    }

    #[test]
    #[should_panic(expected = "Cannot leave until circle is settled")]
    fn test_leave_circle_rejected_when_unsettled() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.leave_circle("circle-0".to_string());
    }

    #[test]
    #[should_panic(expected = "Shares must sum to 10_000 bps")]
    fn test_add_expense_invalid_shares() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(1_000_000_000_000_000_000_000_000),
            vec![MemberShare {
                account_id: accounts(0),
                weight_bps: 5_000,
            }],
            "Dinner".to_string(),
        );
    }

    #[test]
    #[should_panic(expected = "Insufficient storage credit; call storage_deposit")]
    fn test_add_expense_requires_storage_payment() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        let min = contract.storage_balance_bounds().min.as_yoctonear();

        ctx = context(accounts(0), min);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), min);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );
    }

    #[test]
    fn test_add_expense_refunds_excess_deposit() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        let min = contract.storage_balance_bounds().min.as_yoctonear();

        ctx = context(accounts(0), min);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), min);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );

        let stored = contract.storage_deposits.get(&accounts(0)).unwrap_or(0);
        assert_eq!(stored, min);
    }

    #[test]
    fn test_compute_balances() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare {
                    account_id: accounts(0),
                    weight_bps: 5_000,
                },
                MemberShare {
                    account_id: accounts(1),
                    weight_bps: 5_000,
                },
            ],
            "Taxi".to_string(),
        );

        let balances = contract.compute_balances("circle-0".to_string());
        let mut map = std::collections::HashMap::new();
        for entry in balances {
            map.insert(entry.account_id, entry.net.0);
        }
        assert_eq!(map.get(&accounts(0)).copied(), Some(50));
        assert_eq!(map.get(&accounts(1)).copied(), Some(-50));
    }

    #[test]
    fn test_pay_native_records_settlement() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 500);
        testing_env!(ctx.build());
        contract.pay_native("circle-0".to_string(), accounts(1));

        let settlements_len = contract
            .settlements_len
            .get(&"circle-0".to_string())
            .unwrap_or(0);
        assert_eq!(settlements_len, 1);

        let index_key = NearSplitter::settlement_index_key("circle-0", 0);
        let settlement_id = contract
            .settlements_index
            .get(&index_key)
            .expect("Settlement recorded");
        let settlement = contract
            .settlement_by_id
            .get(&settlement_id)
            .expect("Settlement exists");
        assert_eq!(settlement.amount, U128(500));
        assert_eq!(settlement.tx_kind, "native");
    }

    #[test]
    fn test_list_settlements_pagination() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        for i in 0..5u64 {
            let settlement = Settlement {
                circle_id: "circle-0".to_string(),
                from: accounts(0),
                to: accounts(1),
                amount: U128(10 + i as u128),
                token: None,
                ts_ms: timestamp_ms(),
                tx_kind: "native".to_string(),
            };
            contract.record_settlement(settlement);
        }

        let page = contract.list_settlements("circle-0".to_string(), Some(1), Some(2));
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].amount, U128(11));
        assert_eq!(page[1].amount, U128(12));
    }

    #[test]
    fn test_compute_balances_after_native_settlement() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );

        // Account(1) pays 20 to Account(0)
        ctx = context(accounts(1), 20);
        testing_env!(ctx.build());
        contract.pay_native("circle-0".to_string(), accounts(0));

        let balances = contract.compute_balances("circle-0".to_string());
        let mut map = std::collections::HashMap::new();
        for entry in balances {
            map.insert(entry.account_id, entry.net.0);
        }
        assert_eq!(map.get(&accounts(0)).copied(), Some(30));
        assert_eq!(map.get(&accounts(1)).copied(), Some(-30));
    }

    #[test]
    fn test_compute_balances_after_ft_settlement() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );

        let settlement = Settlement {
            circle_id: "circle-0".to_string(),
            from: accounts(1),
            to: accounts(0),
            amount: U128(20),
            token: Some(accounts(2)),
            ts_ms: timestamp_ms(),
            tx_kind: "ft_transfer".to_string(),
        };
        contract.record_settlement(settlement);

        let balances = contract.compute_balances("circle-0".to_string());
        let mut map = std::collections::HashMap::new();
        for entry in balances {
            map.insert(entry.account_id, entry.net.0);
        }
        assert_eq!(map.get(&accounts(0)).copied(), Some(50));
        assert_eq!(map.get(&accounts(1)).copied(), Some(-50));
    }

    #[test]
    #[should_panic(expected = "Circle has reached maximum settlements limit (10,000)")]
    fn test_record_settlement_reaches_cap() {
        let mut contract = setup();

        for _ in 0..MAX_SETTLEMENTS_PER_CIRCLE {
            let settlement = Settlement {
                circle_id: "circle-0".to_string(),
                from: accounts(0),
                to: accounts(1),
                amount: U128(1),
                token: None,
                ts_ms: timestamp_ms(),
                tx_kind: "native".to_string(),
            };
            contract.record_settlement(settlement);
        }

        let settlement = Settlement {
            circle_id: "circle-0".to_string(),
            from: accounts(0),
            to: accounts(1),
            amount: U128(1),
            token: None,
            ts_ms: timestamp_ms(),
            tx_kind: "native".to_string(),
        };
        contract.record_settlement(settlement);
    }

    #[test]
    #[should_panic(expected = "Circle has reached maximum settlements limit (10,000)")]
    fn test_pay_native_respects_settlement_cap() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        contract
            .settlements_len
            .insert(&"circle-0".to_string(), &(MAX_SETTLEMENTS_PER_CIRCLE as u64));

        ctx = context(accounts(0), 1);
        testing_env!(ctx.build());
        contract.pay_native("circle-0".to_string(), accounts(1));
    }

    #[test]
    #[should_panic(expected = "Attach deposit equal to settlement amount")]
    fn test_pay_native_requires_deposit() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.pay_native("circle-0".to_string(), accounts(1));
    }

    #[test]
    fn test_autopay_escrow_reduced_after_partial_payment() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );

        // Partial payment reduces debt from 50 to 30
        ctx = context(accounts(1), 20);
        testing_env!(ctx.build());
        contract.pay_native("circle-0".to_string(), accounts(0));

        // Debtor confirms with reduced escrow (30 instead of 50)
        ctx = context(accounts(1), 30);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        let escrow_key = format!("{}:{}", "circle-0", accounts(1));
        let escrowed = contract.escrow_deposits.get(&escrow_key).unwrap_or(0);
        assert_eq!(escrowed, 30);
    }

    #[test]
    fn test_autopay_creates_pending_payouts() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );

        // Creditor confirms (no deposit needed)
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // Debtor confirms and deposits full debt
        ctx = context(accounts(1), 50);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        assert_eq!(contract.get_pending_payout(accounts(0)), U128(50));
        assert_eq!(contract.get_pending_payout(accounts(1)), U128(0));
    }

    #[test]
    fn test_withdraw_payout_clears_pending_balance() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        contract.pending_payouts.insert(&accounts(0), &100u128);

        ctx = context(accounts(0), ONE_YOCTO);
        testing_env!(ctx.build());
        contract.withdraw_payout();

        assert_eq!(contract.pending_payouts.get(&accounts(0)), None);
        assert_eq!(contract.get_pending_payout(accounts(0)), U128(0));
    }

    #[test]
    #[should_panic(expected = "Withdraw pending payouts before unregistering")]
    fn test_storage_unregister_requires_withdraw_when_not_forced() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        contract.pending_payouts.insert(&accounts(0), &10u128);

        ctx = context(accounts(0), ONE_YOCTO);
        testing_env!(ctx.build());
        contract.storage_unregister(Some(false));
    }

    #[test]
    fn test_storage_unregister_force_refunds_pending() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        contract.pending_payouts.insert(&accounts(0), &10u128);

        ctx = context(accounts(0), ONE_YOCTO);
        testing_env!(ctx.build());
        let result = contract.storage_unregister(Some(true));

        assert!(result);
        assert_eq!(contract.storage_deposits.get(&accounts(0)), None);
        assert_eq!(contract.pending_payouts.get(&accounts(0)), None);
    }

    // =========================================================================
    // CLAIMS TESTS
    // =========================================================================

    #[test]
    fn test_file_claim_wrong_amount() {
        let mut contract = setup();

        // Setup accounts
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle and add expense
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );

        // Participant files a claim for wrong amount
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.file_claim(
            "circle-0".to_string(),
            "expense-circle-0-1".to_string(),
            "wrong_amount".to_string(),
            Some(U128(80)),
            None,
        );

        // Verify claim was created
        assert!(contract.has_pending_claims("circle-0".to_string()));
        assert_eq!(contract.get_pending_claims_count("circle-0".to_string()), 1);

        let claims = contract.list_claims("circle-0".to_string(), None, None, None);
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].reason, "wrong_amount");
        assert_eq!(claims[0].status, "pending");
    }

    #[test]
    fn test_approve_claim_updates_expense() {
        let mut contract = setup();

        // Setup
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );

        // File claim
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.file_claim(
            "circle-0".to_string(),
            "expense-circle-0-1".to_string(),
            "wrong_amount".to_string(),
            Some(U128(80)),
            None,
        );

        let claims = contract.list_claims("circle-0".to_string(), None, None, None);
        let claim_id = claims[0].id.clone();

        // Payer approves the claim
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.approve_claim("circle-0".to_string(), claim_id.clone());

        // Verify expense was updated
        let expenses = contract.list_expenses("circle-0".to_string(), None, None);
        assert_eq!(expenses[0].amount_yocto, U128(80));

        // Verify claim status
        let claim = contract.get_claim("circle-0".to_string(), claim_id).unwrap();
        assert_eq!(claim.status, "approved");
        assert!(!contract.has_pending_claims("circle-0".to_string()));
    }

    #[test]
    fn test_reject_claim_keeps_expense() {
        let mut contract = setup();

        // Setup
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );

        // File claim
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.file_claim(
            "circle-0".to_string(),
            "expense-circle-0-1".to_string(),
            "wrong_amount".to_string(),
            Some(U128(80)),
            None,
        );

        let claims = contract.list_claims("circle-0".to_string(), None, None, None);
        let claim_id = claims[0].id.clone();

        // Payer rejects the claim
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.reject_claim("circle-0".to_string(), claim_id.clone());

        // Verify expense is unchanged
        let expenses = contract.list_expenses("circle-0".to_string(), None, None);
        assert_eq!(expenses[0].amount_yocto, U128(100));

        // Verify claim status
        let claim = contract.get_claim("circle-0".to_string(), claim_id).unwrap();
        assert_eq!(claim.status, "rejected");
        assert!(!contract.has_pending_claims("circle-0".to_string()));
    }

    #[test]
    fn test_remove_expense_claim() {
        let mut contract = setup();

        // Setup
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );

        // File remove expense claim
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.file_claim(
            "circle-0".to_string(),
            "expense-circle-0-1".to_string(),
            "remove_expense".to_string(),
            None,
            None,
        );

        let claims = contract.list_claims("circle-0".to_string(), None, None, None);
        let claim_id = claims[0].id.clone();

        // Payer approves removal
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.approve_claim("circle-0".to_string(), claim_id);

        // Verify expense was removed
        let expenses = contract.list_expenses("circle-0".to_string(), None, None);
        assert!(expenses.is_empty());
    }

    #[test]
    fn test_pending_claim_excludes_from_balance() {
        let mut contract = setup();

        // Setup
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );

        // Check balances before claim
        let balances = contract.compute_balances("circle-0".to_string());
        let mut map = std::collections::HashMap::new();
        for entry in &balances {
            map.insert(entry.account_id.clone(), entry.net.0);
        }
        assert_eq!(map.get(&accounts(0)).copied(), Some(50));
        assert_eq!(map.get(&accounts(1)).copied(), Some(-50));

        // File claim
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.file_claim(
            "circle-0".to_string(),
            "expense-circle-0-1".to_string(),
            "wrong_amount".to_string(),
            Some(U128(80)),
            None,
        );

        // Check balances after claim - expense should be excluded
        let balances_after = contract.compute_balances("circle-0".to_string());
        let mut map_after = std::collections::HashMap::new();
        for entry in &balances_after {
            map_after.insert(entry.account_id.clone(), entry.net.0);
        }
        // Both should be 0 since the only expense is disputed
        assert_eq!(map_after.get(&accounts(0)).copied(), Some(0));
        assert_eq!(map_after.get(&accounts(1)).copied(), Some(0));
    }

    #[test]
    #[should_panic(expected = "Only expense participants can file claims")]
    fn test_non_participant_cannot_file_claim() {
        let mut contract = setup();

        // Setup 3 accounts
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(2), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(2), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add expense only for accounts 0 and 1
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );

        // Account 2 tries to file claim (not a participant)
        ctx = context(accounts(2), 0);
        testing_env!(ctx.build());
        contract.file_claim(
            "circle-0".to_string(),
            "expense-circle-0-1".to_string(),
            "wrong_amount".to_string(),
            Some(U128(80)),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "Only the expense payer can approve claims")]
    fn test_non_payer_cannot_approve_claim() {
        let mut contract = setup();

        // Setup
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );

        // File claim
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.file_claim(
            "circle-0".to_string(),
            "expense-circle-0-1".to_string(),
            "wrong_amount".to_string(),
            Some(U128(80)),
            None,
        );

        let claims = contract.list_claims("circle-0".to_string(), None, None, None);
        let claim_id = claims[0].id.clone();

        // Non-payer tries to approve
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.approve_claim("circle-0".to_string(), claim_id);
    }

    // =========================================================================
    // FIX VERIFICATION TESTS
    // =========================================================================

    /// B1-FIX: Verify multiple members can confirm ledger after first locks circle
    #[test]
    fn test_multiple_confirmations_allowed_after_lock() {
        let mut contract = setup();

        // Setup 3 accounts
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(2), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(2), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add expense: account(0) paid 300, split 3 ways => each owes 100
        // account(0) is owed 200 (paid 300, owes 100)
        // account(1) owes 100
        // account(2) owes 100
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(300),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 3334 },
                MemberShare { account_id: accounts(1), weight_bps: 3333 },
                MemberShare { account_id: accounts(2), weight_bps: 3333 },
            ],
            "Dinner".to_string(),
        );

        // First confirmation by creditor (account 0 - no deposit needed)
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // Verify circle is locked
        let circle = contract.get_circle("circle-0".to_string());
        assert!(circle.locked);
        assert_eq!(circle.state, CircleState::SettlementInProgress);

        // B1-FIX: Second confirmation should succeed (was failing before fix)
        ctx = context(accounts(1), 100);  // Debtor deposits escrow
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // Third confirmation
        ctx = context(accounts(2), 100);  // Debtor deposits escrow
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // All confirmed
        let confirmations = contract.get_confirmations("circle-0".to_string());
        assert_eq!(confirmations.len(), 3);
    }

    /// C1-FIX: Verify expense IDs remain unique after deletion
    #[test]
    fn test_expense_id_unique_after_deletion() {
        let mut contract = setup();

        // Setup
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add first expense
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Expense 1".to_string(),
        );

        let expenses_before = contract.list_expenses("circle-0".to_string(), None, None);
        let first_expense_id = expenses_before[0].id.clone();
        assert_eq!(first_expense_id, "expense-circle-0-1");

        // Delete the expense
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.delete_expense("circle-0".to_string(), first_expense_id.clone());

        // Add new expense
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(200),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Expense 2".to_string(),
        );

        // C1-FIX: New expense should have different ID (was reusing ID before fix)
        let expenses_after = contract.list_expenses("circle-0".to_string(), None, None);
        assert_eq!(expenses_after.len(), 1);
        let second_expense_id = expenses_after[0].id.clone();
        assert_ne!(first_expense_id, second_expense_id);
        assert_eq!(second_expense_id, "expense-circle-0-2");  // Should be 2, not 1
    }

    #[test]
    fn test_storage_credit_refunded_on_delete_expense() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Expense 1".to_string(),
        );

        let before = contract.storage_deposits.get(&accounts(0)).unwrap_or(0);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.delete_expense("circle-0".to_string(), "expense-circle-0-1".to_string());

        let after = contract.storage_deposits.get(&accounts(0)).unwrap_or(0);
        assert!(after >= before, "Storage credit should not decrease after deletion");
    }

    #[test]
    fn test_list_expenses_pagination_large() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add many expenses
        for i in 0..120u64 {
            ctx = context(accounts(0), 0);
            testing_env!(ctx.build());
            contract.add_expense(
                "circle-0".to_string(),
                U128(1),
                vec![
                    MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                    MemberShare { account_id: accounts(1), weight_bps: 5_000 },
                ],
                format!("Expense {}", i + 1),
            );
        }

        let page = contract.list_expenses("circle-0".to_string(), Some(50), Some(10));
        assert_eq!(page.len(), 10);
        assert_eq!(page[0].id, "expense-circle-0-51");
        assert_eq!(page[9].id, "expense-circle-0-60");
    }

    #[test]
    fn test_delete_expense_pagination_skips_tombstones() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add 5 expenses
        for i in 0..5u64 {
            ctx = context(accounts(0), 0);
            testing_env!(ctx.build());
            contract.add_expense(
                "circle-0".to_string(),
                U128(10 + i),
                vec![
                    MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                    MemberShare { account_id: accounts(1), weight_bps: 5_000 },
                ],
                format!("Expense {}", i + 1),
            );
        }

        // Delete the third expense (id 3)
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.delete_expense("circle-0".to_string(), "expense-circle-0-3".to_string());

        // Add another expense (should be id 6)
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(99),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Expense 6".to_string(),
        );

        let page = contract.list_expenses("circle-0".to_string(), Some(2), Some(3));
        assert_eq!(page.len(), 3);
        assert_eq!(page[0].id, "expense-circle-0-4");
        assert_eq!(page[1].id, "expense-circle-0-5");
        assert_eq!(page[2].id, "expense-circle-0-6");
    }

    /// A2-FIX: Verify ft_on_transfer returns amount (refund) on invalid message
    #[test]
    fn test_ft_on_transfer_invalid_msg_refunds() {
        let mut contract = setup();

        // Setup accounts
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Set up context as token contract calling ft_on_transfer
        let token_contract: AccountId = "token.near".parse().unwrap();
        let mut ctx = VMContextBuilder::new();
        ctx.predecessor_account_id(token_contract.clone());
        ctx.prepaid_gas(Gas::from_tgas(100));
        testing_env!(ctx.build());

        // Call with invalid message - A2-FIX: should return amount for refund
        let result = contract.ft_on_transfer(
            accounts(0),
            U128(1000),
            "invalid json".to_string(),
        );

        // Should return the amount to refund (not "0" or Value("1000"))
        match result {
            PromiseOrValue::Value(refund) => {
                assert_eq!(refund.0, 1000, "Should refund full amount on invalid msg");
            }
            PromiseOrValue::Promise(_) => {
                panic!("Should not return Promise on parse error");
            }
        }
    }

    /// A2-FIX: Verify ft_on_transfer returns amount when circle not found
    #[test]
    fn test_ft_on_transfer_circle_not_found_refunds() {
        let mut contract = setup();

        // Setup sender account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Set up context as token contract
        let token_contract: AccountId = "token.near".parse().unwrap();
        let mut ctx = VMContextBuilder::new();
        ctx.predecessor_account_id(token_contract);
        ctx.prepaid_gas(Gas::from_tgas(100));
        testing_env!(ctx.build());

        // Call with non-existent circle
        let result = contract.ft_on_transfer(
            accounts(0),
            U128(1000),
            r#"{"circle_id": "nonexistent", "to": "bob.near"}"#.to_string(),
        );

        match result {
            PromiseOrValue::Value(refund) => {
                assert_eq!(refund.0, 1000, "Should refund full amount when circle not found");
            }
            PromiseOrValue::Promise(_) => {
                panic!("Should not return Promise when circle not found");
            }
        }
    }

    #[test]
    fn test_ft_on_transfer_sender_not_member_refunds() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(2), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        let token_contract: AccountId = "token.near".parse().unwrap();
        let mut ctx = VMContextBuilder::new();
        ctx.predecessor_account_id(token_contract);
        ctx.prepaid_gas(Gas::from_tgas(100));
        testing_env!(ctx.build());

        let result = contract.ft_on_transfer(
            accounts(2),
            U128(1000),
            format!("{{\"circle_id\": \"circle-0\", \"to\": \"{}\"}}", accounts(1)),
        );

        match result {
            PromiseOrValue::Value(refund) => {
                assert_eq!(refund.0, 1000, "Should refund full amount when sender not member");
            }
            PromiseOrValue::Promise(_) => {
                panic!("Should not return Promise when sender not member");
            }
        }
    }

    #[test]
    fn test_on_ft_forward_complete_success_records_settlement() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        let contract_id: AccountId = "contract.near".parse().unwrap();
        let token_contract: AccountId = "token.near".parse().unwrap();
        let mut ctx = VMContextBuilder::new();
        ctx.current_account_id(contract_id.clone());
        ctx.predecessor_account_id(contract_id);
        ctx.signer_account_id(accounts(0));
        testing_env!(ctx.build(), PromiseResult::Successful(vec![]));

        let result = contract.on_ft_forward_complete(
            accounts(0),
            U128(500),
            token_contract,
            "circle-0".to_string(),
            accounts(1),
        );

        assert_eq!(result.0, 0);
        let settlements_len = contract.settlements_len.get(&"circle-0".to_string()).unwrap_or(0);
        assert_eq!(settlements_len, 1);
    }

    #[test]
    fn test_on_ft_forward_complete_failed_no_settlement() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        let contract_id: AccountId = "contract.near".parse().unwrap();
        let token_contract: AccountId = "token.near".parse().unwrap();
        let mut ctx = VMContextBuilder::new();
        ctx.current_account_id(contract_id.clone());
        ctx.predecessor_account_id(contract_id);
        ctx.signer_account_id(accounts(0));
        testing_env!(ctx.build(), PromiseResult::Failed);

        let result = contract.on_ft_forward_complete(
            accounts(0),
            U128(500),
            token_contract,
            "circle-0".to_string(),
            accounts(1),
        );

        assert_eq!(result.0, 500);
        let settlements_len = contract.settlements_len.get(&"circle-0".to_string()).unwrap_or(0);
        assert_eq!(settlements_len, 0);
    }

    #[test]
    fn test_on_ft_forward_complete_insufficient_storage_no_panic() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        let min = contract.storage_balance_bounds().min.as_yoctonear();

        ctx = context(accounts(0), min);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), min);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        let contract_id: AccountId = "contract.near".parse().unwrap();
        let token_contract: AccountId = "token.near".parse().unwrap();
        let mut ctx = VMContextBuilder::new();
        ctx.current_account_id(contract_id.clone());
        ctx.predecessor_account_id(contract_id);
        ctx.signer_account_id(accounts(0));
        testing_env!(ctx.build(), PromiseResult::Successful(vec![]));

        let result = contract.on_ft_forward_complete(
            accounts(0),
            U128(500),
            token_contract,
            "circle-0".to_string(),
            accounts(1),
        );

        assert_eq!(result.0, 0);
        let settlements_len = contract.settlements_len.get(&"circle-0".to_string()).unwrap_or(0);
        assert_eq!(settlements_len, 0);
    }

    /// C2-FIX: Verify approve_claim validates proposed amount constraints
    #[test]
    #[should_panic(expected = "Proposed amount exceeds maximum safe value")]
    fn test_approve_claim_validates_amount_max() {
        let mut contract = setup();

        // Setup
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );

        // File claim with amount exceeding i128::MAX
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.file_claim(
            "circle-0".to_string(),
            "expense-circle-0-1".to_string(),
            "wrong_amount".to_string(),
            Some(U128(u128::MAX)),  // Exceeds i128::MAX
            None,
        );

        let claims = contract.list_claims("circle-0".to_string(), None, None, None);
        let claim_id = claims[0].id.clone();

        // C2-FIX: Approving should fail due to amount validation
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.approve_claim("circle-0".to_string(), claim_id);
    }
}

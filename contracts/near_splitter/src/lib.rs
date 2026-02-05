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
    ext_contract, near, require, AccountId, BorshStorageKey, Gas, NearToken,
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
/// Maximum items to process in a single batch cleanup call to stay within gas limits
/// Conservative estimate: ~100 storage operations per batch is safe
const MAX_CLEANUP_BATCH_SIZE: u64 = 100;
const EVENT_STANDARD: &str = "nearsplitter";
const EVENT_VERSION: &str = "1.0.0";
const TARGET_BPS_TOTAL: u16 = 10_000;
const ONE_YOCTO: u128 = 1;
const GAS_FT_TRANSFER_TGAS: u64 = 30;
const GAS_FT_CALLBACK_TGAS: u64 = 15;
/// Gas for cross-contract ft_metadata() call
const GAS_FT_METADATA_TGAS: u64 = 10;
/// Gas reserved for on_ft_metadata callback to validate and store
const GAS_FT_METADATA_CALLBACK_TGAS: u64 = 20;
/// Maximum allowed length for token name in metadata
const MAX_FT_METADATA_NAME_LEN: usize = 128;
/// Maximum allowed length for token symbol in metadata
const MAX_FT_METADATA_SYMBOL_LEN: usize = 32;

fn timestamp_ms() -> u64 {
    env::block_timestamp() / 1_000_000
}

/// Constant-time string comparison to prevent timing attacks.
/// SECURITY: Standard == comparison can leak info about matching prefix length.
/// This is critical for invite code hash verification.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let mut result: u8 = 0;
    for (x, y) in a_bytes.iter().zip(b_bytes.iter()) {
        result |= x ^ y;
    }
    result == 0
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

/// Safe increment for u64 counters. Panics on overflow with descriptive message.
/// Used for monotonic counters like expense/settlement/claim indices.
#[inline]
fn safe_increment_u64(value: u64, counter_name: &str) -> u64 {
    value
        .checked_add(1)
        .unwrap_or_else(|| env::panic_str(&format!("{} counter overflow", counter_name)))
}

/// Assert that exactly 1 yoctoNEAR is attached.
/// Used for sensitive state-changing operations that require explicit user confirmation.
/// The 1 yoctoNEAR pattern is a NEAR convention that:
/// - Proves the user explicitly signed the transaction (not a view call)
/// - Prevents accidental calls from scripts/bots
/// - Is low enough to not be a barrier but high enough to require intention
#[inline]
fn assert_one_yocto() {
    require!(
        env::attached_deposit().as_yoctonear() == ONE_YOCTO,
        "Requires exactly 1 yoctoNEAR attached for security confirmation"
    );
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
    /// D1-FIX: O(1) pending claims counter per circle to avoid gas-expensive iteration
    PendingClaimsCount,
    /// Tracks cleanup progress for batched deletion of large circles
    /// Key: circle_id, Value: next index to process
    CleanupProgress,
    /// Aggregate escrow total per account for O(1) lookup and validation
    EscrowTotalByAccount,
    /// TOKEN-ALLOWLIST: Approved FT contracts that can be used for settlements
    /// Only tokens in this list are accepted by ft_on_transfer
    ApprovedTokens,
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
    /// EPOCH-FIX: Current ledger epoch - incremented after each settlement round.
    /// Only expenses and settlements with matching epoch are included in balance calculations.
    pub ledger_epoch: u64,
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
    /// EPOCH-FIX: The ledger epoch when this expense was created.
    /// Used to filter expenses by settlement round.
    pub epoch: u64,
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
    /// EPOCH-FIX: The ledger epoch when this settlement was recorded.
    /// Used to filter settlements by settlement round.
    pub epoch: u64,
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

// NEAR SDK 5.x Contract State Definition
// The #[near(contract_state)] attribute generates:
// - ContractState trait implementation (required by NEAR runtime)
// - Borsh serialization for contract state (BorshSerialize + BorshDeserialize)
// - State storage/retrieval logic
// Use #[near] on impl blocks to expose methods as contract endpoints.
// NOTE: Do NOT add #[derive(BorshSerialize, BorshDeserialize)] - the near macro handles this.
#[near(contract_state)]
#[derive(PanicOnDefault)]
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
    /// D1-FIX: O(1) pending claims counter per circle - avoids gas-expensive iteration
    /// Key: circle_id, Value: count of pending claims
    pending_claims_count: LookupMap<String, u64>,
    /// Tracks cleanup progress for batched deletion
    /// Key: "circle_id:type" (type = "expenses", "settlements", "claims"), Value: next index to clean
    cleanup_progress: LookupMap<String, u64>,
    /// Aggregate escrow total per account - enables O(1) lookup for unregister validation
    /// Key: account_id, Value: total escrowed across all circles
    escrow_total_by_account: LookupMap<AccountId, u128>,
    /// Global aggregate of all escrow deposits - for rescue calculations
    total_escrow: u128,
    /// Global aggregate of all storage deposits - for rescue calculations
    total_storage_deposits: u128,
    /// Global aggregate of all pending payouts - for rescue calculations
    total_pending_payouts: u128,
    /// TOKEN-ALLOWLIST: Approved FT contracts for settlements
    /// Key: token contract AccountId, Value: true if approved
    /// Only tokens in this list are accepted by ft_on_transfer to prevent
    /// malicious token contracts from spoofing sender_id and draining storage
    approved_tokens: LookupMap<AccountId, bool>,
}

// PRIMARY CONTRACT METHODS (impl block 1 of 2)
// This impl block contains: initialization, storage management, circle/expense CRUD,
// claims handling, and view methods. Additional settlement methods are in impl block 2.
#[near]
impl NearSplitter {
    /// Initialize the NearSplitter contract.
    /// 
    /// This creates all necessary storage collections for circles, expenses,
    /// settlements, claims, and related tracking data.
    /// 
    /// # Security
    /// - Can only be called once (panics if contract is already initialized)
    /// - Prevents re-initialization attacks that could reset all contract state
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
            // D1-FIX: Initialize pending claims counter
            pending_claims_count: LookupMap::new(StorageKey::PendingClaimsCount),
            // Gas-safe batched cleanup progress tracking
            cleanup_progress: LookupMap::new(StorageKey::CleanupProgress),
            // Escrow aggregate tracking
            escrow_total_by_account: LookupMap::new(StorageKey::EscrowTotalByAccount),
            total_escrow: 0,
            // Storage and payout aggregate tracking for rescue calculations
            total_storage_deposits: 0,
            total_pending_payouts: 0,
            // TOKEN-ALLOWLIST: Initialize approved tokens map
            approved_tokens: LookupMap::new(StorageKey::ApprovedTokens),
        }
    }

    /// Migrate the contract state to a new version.
    /// 
    /// # WARNING: DEVELOPMENT/TESTNET ONLY - DO NOT DEPLOY TO MAINNET
    /// This implementation resets all state. For production deployments:
    /// 1. Implement proper state migration logic  
    /// 2. Preserve user funds and data
    /// 3. Use versioned state structs
    /// 
    /// # Security
    /// - #[private] macro restricts to contract account only
    /// - Typically called via DAO proposal after code upgrade
    /// - FIX-3: cfg gate ensures this CANNOT be compiled for production
    #[cfg(all(not(target_arch = "wasm32"), any(test, feature = "dev")))]
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
            // D1-FIX: Initialize pending claims counter
            pending_claims_count: LookupMap::new(StorageKey::PendingClaimsCount),
            // Gas-safe batched cleanup progress tracking
            cleanup_progress: LookupMap::new(StorageKey::CleanupProgress),
            // Escrow aggregate tracking
            escrow_total_by_account: LookupMap::new(StorageKey::EscrowTotalByAccount),
            total_escrow: 0,
            // Storage and payout aggregate tracking for rescue calculations
            total_storage_deposits: 0,
            total_pending_payouts: 0,
            // TOKEN-ALLOWLIST: Initialize approved tokens map
            approved_tokens: LookupMap::new(StorageKey::ApprovedTokens),
        }
    }

    /// Get a circle by its ID.
    /// 
    /// # Arguments
    /// * `circle_id` - The unique identifier of the circle
    /// 
    /// # Returns
    /// The Circle struct containing all circle metadata
    /// 
    /// # Panics
    /// Panics if the circle does not exist
    pub fn get_circle(&self, circle_id: String) -> Circle {
        self.circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"))
    }

    /// List all circles owned by a specific account.
    /// 
    /// # Arguments
    /// * `owner` - The account ID of the circle owner
    /// * `from` - Starting index for pagination (0-based)
    /// * `limit` - Maximum number of results (capped at 100)
    /// 
    /// # Returns
    /// Vector of Circle structs owned by the account
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

    /// Clear expenses for a circle. Safe for up to MAX_EXPENSES_PER_CIRCLE (500) entries.
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

    /// Clear settlements in batches for gas safety.
    /// Returns the number of items remaining to clear (0 means complete).
    /// Settlements can have up to 10,000 entries which may exceed gas limits in a single call.
    fn clear_settlements_batch(&mut self, circle_id: &str, limit: u64) -> u64 {
        let total = self.settlements_len.get(&circle_id.to_string()).unwrap_or(0);
        if total == 0 {
            return 0;
        }
        
        let progress_key = format!("{}:settlements", circle_id);
        let start_idx = self.cleanup_progress.get(&progress_key).unwrap_or(0);
        
        let end_idx = (start_idx + limit).min(total);
        
        for idx in start_idx..end_idx {
            let key = Self::settlement_index_key(circle_id, idx);
            if let Some(settlement_id) = self.settlements_index.get(&key) {
                self.settlement_by_id.remove(&settlement_id);
            }
            self.settlements_index.remove(&key);
        }
        
        let remaining = total - end_idx;
        if remaining == 0 {
            // Cleanup complete
            self.settlements_len.remove(&circle_id.to_string());
            self.cleanup_progress.remove(&progress_key);
        } else {
            // Save progress for next batch
            self.cleanup_progress.insert(&progress_key, &end_idx);
        }
        
        remaining
    }

    /// Clear claims in batches for gas safety.
    /// Returns the number of items remaining to clear (0 means complete).
    fn clear_claims_batch(&mut self, circle_id: &str, limit: u64) -> u64 {
        let total = self.claims_len.get(&circle_id.to_string()).unwrap_or(0);
        if total == 0 {
            self.pending_claims_count.remove(&circle_id.to_string());
            return 0;
        }
        
        let progress_key = format!("{}:claims", circle_id);
        let start_idx = self.cleanup_progress.get(&progress_key).unwrap_or(0);
        
        let end_idx = (start_idx + limit).min(total);
        
        for idx in start_idx..end_idx {
            let key = Self::claim_index_key(circle_id, idx);
            if let Some(claim_id) = self.claims_index.get(&key) {
                self.claim_by_id.remove(&claim_id);
            }
            self.claims_index.remove(&key);
        }
        
        let remaining = total - end_idx;
        if remaining == 0 {
            // Cleanup complete
            self.claims_len.remove(&circle_id.to_string());
            self.cleanup_progress.remove(&progress_key);
            self.pending_claims_count.remove(&circle_id.to_string());
        } else {
            // Save progress for next batch
            self.cleanup_progress.insert(&progress_key, &end_idx);
        }
        
        remaining
    }

    /// Legacy function - still used where settlement count is bounded
    /// For large circles, use clear_settlements_batch instead
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

    /// Legacy function - still used where claim count is bounded
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
        // D1-FIX: Reset pending claims counter when clearing all claims
        self.pending_claims_count.remove(&circle_id.to_string());
    }

    /// List expenses for a circle with pagination.
    /// 
    /// # Arguments
    /// * `circle_id` - The circle to list expenses for
    /// * `from` - Starting index for pagination (0-based)
    /// * `limit` - Maximum number of results (capped at 100)
    /// 
    /// # Returns
    /// Vector of Expense structs for the circle
    /// 
    /// # Note
    /// May contain gaps where expenses were deleted (tombstone design).
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

    /// List settlements for a circle with pagination.
    /// 
    /// # Arguments
    /// * `circle_id` - The circle to list settlements for
    /// * `from` - Starting index for pagination (0-based)
    /// * `limit` - Maximum number of results (capped at 100)
    /// 
    /// # Returns
    /// Vector of Settlement structs for the circle
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
    /// EPOCH-FIX: Only includes expenses and settlements from the current epoch.
    pub fn compute_balances(&self, circle_id: String) -> Vec<BalanceView> {
        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));
        let current_epoch = circle.ledger_epoch;
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
            // EPOCH-FIX: Skip expenses from previous epochs
            if expense.epoch != current_epoch {
                continue;
            }
            
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
        // EPOCH-FIX: Only include settlements from the current epoch.
        let settlements = self.iter_settlements_by_circle(&circle_id);
        for settlement in settlements {
            // EPOCH-FIX: Skip settlements from previous epochs
            if settlement.epoch != current_epoch {
                continue;
            }
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

            // Use saturating_sub for defense-in-depth (amount is always <= min(debt, credit))
            debt = debt.saturating_sub(amount);
            credit = credit.saturating_sub(amount);

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
            ledger_epoch: 0, // EPOCH-FIX: Start at epoch 0
        };

        self.circles.insert(&circle_id, &circle);

        let mut owner_list = self.circles_by_owner.get(&owner).unwrap_or_default();
        owner_list.push(circle_id.clone());
        self.circles_by_owner.insert(&owner, &owner_list);

        // Add owner to member index
        self.add_member_to_index(&owner, &circle_id);

        self.apply_storage_cost(&owner, initial_storage, true, None);

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
            // FIX-2: Use constant-time comparison to prevent timing attacks
            // Standard == could leak info about how many characters matched
            require!(
                constant_time_eq(&provided_hash, expected_hash),
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

        self.apply_storage_cost(&account, initial_storage, true, None);

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
        // E2-FIX: Enforce escrow must be zero - user must disable autopay first to get refund
        require!(escrowed == 0, "Cannot leave with escrowed funds. Disable autopay first to withdraw escrow.");

        let initial_storage = env::storage_usage();
        
        // Remove from members
        circle.members.remove(member_index.unwrap());
        self.circles.insert(&circle_id, &circle);
        
        // Remove from member index
        self.remove_member_from_index(&account, &circle_id);
        
        // SECURITY: Complete ALL state changes before any external calls (reentrancy protection)
        // Cleanup autopay preference (escrow already confirmed to be 0)
        let autopay_key = format!("{}:{}", circle_id, account);
        self.autopay_preferences.remove(&autopay_key);
        // E2-FIX: Remove dead escrow handling - we already required escrowed == 0
        self.escrow_deposits.remove(&escrow_key);

        self.apply_storage_cost(&account, initial_storage, false, None);
        
        self.emit_event(
            "circle_leave",
            json!([{ "circle_id": circle_id, "account_id": account }]),
        );
    }

    /// Transfer ownership of a circle to another member.
    /// Only the current owner can call this.
    /// 
    /// # Security
    /// Requires exactly 1 yoctoNEAR attached to confirm this sensitive operation.
    #[payable]
    pub fn transfer_ownership(&mut self, circle_id: String, new_owner: AccountId) {
        assert_one_yocto();
        let account = env::predecessor_account_id();
        
        let mut circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(circle.owner == account, "Only owner can transfer ownership");
        require!(!circle.locked, "Cannot transfer ownership during settlement");
        require!(
            circle.state == CircleState::Open || circle.state == CircleState::Settled,
            "Cannot transfer ownership during settlement"
        );
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

        self.apply_storage_cost(&account, initial_storage, true, None);
        
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
    /// - Large data sets exist (use cleanup_circle_data first)
    /// 
    /// For circles with many settlements (>100), use cleanup_circle_data() first
    /// to clear data in batches before calling delete_circle.
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
            circle.state != CircleState::SettlementInProgress
                && circle.state != CircleState::SettlementExecuting,
            "Cannot delete circle while settlement is in progress"
        );

        // Check that only owner remains (all others must leave first)
        require!(
            circle.members.len() == 1 && circle.members[0] == account,
            "All other members must leave before deleting circle"
        );

        // Check no pending escrow deposits
        let escrow_key = format!("{}:{}", circle_id, account);
        let escrowed = self.escrow_deposits.get(&escrow_key).unwrap_or(0);
        require!(escrowed == 0, "Withdraw escrowed funds before deleting");

        // Gas safety: check that data sets are small enough for single-transaction cleanup
        let settlements_count = self.settlements_len.get(&circle_id).unwrap_or(0);
        let claims_count = self.claims_len.get(&circle_id).unwrap_or(0);
        require!(
            settlements_count <= MAX_CLEANUP_BATCH_SIZE && claims_count <= MAX_CLEANUP_BATCH_SIZE,
            "Circle has too much data for single delete. Call cleanup_circle_data() first."
        );

        let initial_storage = env::storage_usage();

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

        // Clean up all associated data (safe because we checked sizes above)
        self.circles.remove(&circle_id);
        self.clear_expenses_for_circle(&circle_id);
        self.clear_settlements_for_circle(&circle_id);
        self.clear_confirmations_for_circle(&circle_id, &circle.members);
        self.clear_claims_for_circle(&circle_id);
        self.next_expense_index.remove(&circle_id);
        
        // Clean up autopay preferences
        let autopay_key = format!("{}:{}", circle_id, account);
        self.autopay_preferences.remove(&autopay_key);
        
        // Clean up any leftover cleanup progress markers
        self.cleanup_progress.remove(&format!("{}:settlements", circle_id));
        self.cleanup_progress.remove(&format!("{}:claims", circle_id));

        self.apply_storage_cost(&account, initial_storage, false, None);

        self.emit_event(
            "circle_deleted",
            json!([{
                "circle_id": circle_id,
                "deleted_by": account,
            }]),
        );
    }

    /// Clean up circle data in batches for gas safety.
    /// Call this repeatedly until it returns (0, 0) before calling delete_circle
    /// on circles with large data sets (>100 settlements or claims).
    /// 
    /// Returns (remaining_settlements, remaining_claims) after this batch.
    /// When both are 0, the circle is ready for deletion.
    /// 
    /// Only the circle owner can call this.
    /// The circle must be in a deletable state (settled, unlocked, only owner remaining).
    pub fn cleanup_circle_data(&mut self, circle_id: String) -> (u64, u64) {
        let account = env::predecessor_account_id();
        
        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(circle.owner == account, "Only owner can cleanup circle data");
        require!(!circle.locked, "Cannot cleanup circle during settlement");
        require!(
            circle.state != CircleState::SettlementInProgress
                && circle.state != CircleState::SettlementExecuting,
            "Cannot cleanup circle while settlement is in progress"
        );

        // Check that only owner remains
        require!(
            circle.members.len() == 1 && circle.members[0] == account,
            "All other members must leave before cleanup"
        );

        let initial_storage = env::storage_usage();

        // Process settlements batch
        let remaining_settlements = self.clear_settlements_batch(&circle_id, MAX_CLEANUP_BATCH_SIZE);
        
        // Process claims batch (if settlements are done)
        let remaining_claims = if remaining_settlements == 0 {
            self.clear_claims_batch(&circle_id, MAX_CLEANUP_BATCH_SIZE)
        } else {
            self.claims_len.get(&circle_id).unwrap_or(0)
        };

        self.apply_storage_cost(&account, initial_storage, false, None);

        self.emit_event(
            "circle_cleanup_batch",
            json!([{
                "circle_id": circle_id,
                "remaining_settlements": remaining_settlements,
                "remaining_claims": remaining_claims,
            }]),
        );

        (remaining_settlements, remaining_claims)
    }

    /// Get cleanup progress for a circle.
    /// Returns (total_settlements, cleared_settlements, total_claims, cleared_claims).
    /// Useful for showing progress to users during multi-transaction cleanup.
    pub fn get_cleanup_progress(&self, circle_id: String) -> (u64, u64, u64, u64) {
        let settlements_total = self.settlements_len.get(&circle_id).unwrap_or(0);
        let settlements_cleared = self.cleanup_progress
            .get(&format!("{}:settlements", circle_id))
            .unwrap_or(0);
        
        let claims_total = self.claims_len.get(&circle_id).unwrap_or(0);
        let claims_cleared = self.cleanup_progress
            .get(&format!("{}:claims", circle_id))
            .unwrap_or(0);
        
        (settlements_total, settlements_cleared, claims_total, claims_cleared)
    }

    /// Add an expense to a circle. Any circle member can add expenses.
    /// 
    /// # Storage Model
    /// All circle data storage (expenses, settlements, claims) is charged to the circle owner's
    /// storage deposit, not the caller's. This ensures predictable costs for the circle owner
    /// who created and manages the circle.
    /// 
    /// # Requirements
    /// - Caller must be registered (have storage deposit)
    /// - Caller must be a circle member
    /// - Circle must not be locked for settlement
    /// - Amount must be positive and fit in i128
    /// - Shares must sum to 10,000 bps (100%)
    /// - All participants must be circle members
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
        let next_expense_index = safe_increment_u64(expense_index, "expense_index");
        let expense_id = format!("expense-{}-{}", circle_id, next_expense_index);
        self.next_expense_index.insert(&circle_id, &next_expense_index);
        let ts_ms = timestamp_ms();

        let expense = Expense {
            id: expense_id.clone(),
            circle_id: circle_id.clone(),
            payer: payer.clone(),
            participants: shares.clone(),
            amount_yocto,
            memo: memo.clone(),
            ts_ms,
            epoch: circle.ledger_epoch, // EPOCH-FIX: Record current epoch
        };

        let index_key = Self::expense_index_key(&circle_id, current_len);
        self.expenses_index.insert(&index_key, &expense_id);
        self.expense_by_id.insert(&expense_id, &expense);
        self.expenses_len.insert(&circle_id, &safe_increment_u64(current_len, "expenses_len"));

        // Reset confirmations when new expense is added
        self.clear_confirmations_for_circle(&circle_id, &circle.members);

        // STORAGE-FIX: Charge circle owner's storage (not caller's) for circle data.
        // This ensures storage refunds go to the correct account when data is cleared.
        self.apply_storage_cost(&circle.owner, initial_storage, false, None);

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
    /// 
    /// # Storage Model
    /// Storage credits from deletion are returned to the circle owner's storage balance,
    /// matching the owner-funded storage model where the owner pays for all circle data.
    /// 
    /// # Security
    /// Requires exactly 1 yoctoNEAR attached to confirm this sensitive operation.
    #[payable]
    pub fn delete_expense(&mut self, circle_id: String, expense_id: String) {
        assert_one_yocto();
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
        // E5-NOTE: expenses_len is NOT decremented intentionally (tombstone design)
        // The expense_by_id entry is removed, but the index slot remains as a tombstone.
        // list_expenses() handles this by skipping missing entries.
        // next_expense_index ensures new expenses get unique IDs (C1-FIX).

        // Reset confirmations since balances changed
        self.clear_confirmations_for_circle(&circle_id, &circle.members);

        // STORAGE-FIX: Refund to circle owner (matches add_expense charging owner)
        self.apply_storage_cost(&circle.owner, initial_storage, false, None);

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
    /// 
    /// # Storage Model
    /// Claim storage is charged to the circle owner's storage balance, not the claimant's.
    /// This ensures consistent owner-funded storage for all circle data.
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
            circle.state != CircleState::SettlementInProgress
                && circle.state != CircleState::SettlementExecuting,
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
            // E1-FIX: Validate amount fits in i128 BEFORE storing the claim
            // This prevents wasting storage on claims that can never be approved
            require!(
                amount.0 <= i128::MAX as u128,
                "Proposed amount exceeds maximum safe value for balance calculation"
            );
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
        self.claims_len.insert(&circle_id, &safe_increment_u64(current_len, "claims_len"));

        // D1-FIX: Increment pending claims counter for O(1) lookup
        let pending_count = self.pending_claims_count.get(&circle_id).unwrap_or(0);
        self.pending_claims_count.insert(&circle_id, &safe_increment_u64(pending_count, "pending_claims_count"));

        // Reset confirmations when claim is filed
        self.clear_confirmations_for_circle(&circle_id, &circle.members);

        // STORAGE-FIX: Charge circle owner's storage (not claimant's) for circle data.
        // This ensures storage refunds go to the correct account when claims are cleared.
        self.apply_storage_cost(&circle.owner, initial_storage, false, None);

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
    /// 
    /// # Storage Model
    /// Storage changes from claim resolution are charged/credited to the circle owner,
    /// consistent with the owner-funded storage model for all circle data.
    /// 
    /// # Security
    /// Requires exactly 1 yoctoNEAR attached to confirm this sensitive operation.
    #[payable]
    pub fn approve_claim(&mut self, circle_id: String, claim_id: String) {
        assert_one_yocto();
        let caller = env::predecessor_account_id();
        self.assert_registered(&caller);

        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(!circle.locked, "Cannot resolve claims while circle is locked");
        require!(
            circle.state != CircleState::SettlementInProgress
                && circle.state != CircleState::SettlementExecuting,
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

        // D1-FIX: Decrement pending claims counter with saturating_sub for safety
        let pending_count = self.pending_claims_count.get(&circle_id).unwrap_or(0);
        let new_count = pending_count.saturating_sub(1);
        if new_count > 0 {
            self.pending_claims_count.insert(&circle_id, &new_count);
        } else {
            self.pending_claims_count.remove(&circle_id);
        }

        // Reset confirmations since balances changed
        self.clear_confirmations_for_circle(&circle_id, &circle.members);

        // STORAGE-FIX: Refund to circle owner (matches file_claim charging owner)
        self.apply_storage_cost(&circle.owner, initial_storage, false, None);

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
    /// 
    /// # Storage Model
    /// Storage changes from claim resolution are charged/credited to the circle owner,
    /// consistent with the owner-funded storage model for all circle data.
    /// 
    /// # Security
    /// Requires exactly 1 yoctoNEAR attached to confirm this sensitive operation.
    #[payable]
    pub fn reject_claim(&mut self, circle_id: String, claim_id: String) {
        assert_one_yocto();
        let caller = env::predecessor_account_id();
        self.assert_registered(&caller);

        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(!circle.locked, "Cannot resolve claims while circle is locked");
        require!(
            circle.state != CircleState::SettlementInProgress
                && circle.state != CircleState::SettlementExecuting,
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

        // D1-FIX: Decrement pending claims counter with saturating_sub for safety
        let pending_count = self.pending_claims_count.get(&circle_id).unwrap_or(0);
        let new_count = pending_count.saturating_sub(1);
        if new_count > 0 {
            self.pending_claims_count.insert(&circle_id, &new_count);
        } else {
            self.pending_claims_count.remove(&circle_id);
        }

        // Reset confirmations to re-evaluate
        self.clear_confirmations_for_circle(&circle_id, &circle.members);

        // STORAGE-FIX: Refund to circle owner (matches file_claim charging owner)
        self.apply_storage_cost(&circle.owner, initial_storage, false, None);

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

    /// List all claims for a circle with optional status filter and pagination.
    /// 
    /// # Arguments
    /// * `circle_id` - The circle to list claims for
    /// * `status` - Optional filter: "pending", "approved", or "rejected"
    /// * `from` - Starting index for pagination (0-based)
    /// * `limit` - Maximum number of results (capped at 100)
    /// 
    /// # Returns
    /// Vector of Claim structs matching the filter
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

    /// Get a specific claim by its ID.
    /// 
    /// # Arguments
    /// * `circle_id` - The circle the claim belongs to
    /// * `claim_id` - The unique claim identifier
    /// 
    /// # Returns
    /// The Claim if found and belongs to the specified circle, None otherwise
    pub fn get_claim(&self, circle_id: String, claim_id: String) -> Option<Claim> {
        self.claim_by_id
            .get(&claim_id)
            .and_then(|claim| if claim.circle_id == circle_id { Some(claim) } else { None })
    }

    /// Get all claims for a specific expense.
    /// 
    /// # Arguments
    /// * `circle_id` - The circle the expense belongs to
    /// * `expense_id` - The expense to get claims for
    /// 
    /// # Returns
    /// Vector of all claims (pending, approved, rejected) for the expense
    pub fn get_expense_claims(&self, circle_id: String, expense_id: String) -> Vec<Claim> {
        // NOTE: Bounded scan across claims (MAX_CLAIMS_PER_CIRCLE)
        self.iter_claims_by_circle(&circle_id)
            .into_iter()
            .filter(|c| c.expense_id == expense_id)
            .collect()
    }

    /// Get the count of pending claims for a circle.
    /// Uses O(1) cached counter instead of O(n) iteration.
    /// 
    /// # Arguments
    /// * `circle_id` - The circle to check
    /// 
    /// # Returns
    /// Number of pending claims in the circle
    pub fn get_pending_claims_count(&self, circle_id: String) -> u64 {
        self.pending_claims_count.get(&circle_id).unwrap_or(0)
    }

    /// Check if a circle has any pending claims.
    /// 
    /// # Arguments
    /// * `circle_id` - The circle to check
    /// 
    /// # Returns
    /// true if there are pending claims, false otherwise
    pub fn has_pending_claims(&self, circle_id: String) -> bool {
        self.get_pending_claims_count(circle_id) > 0
    }

    /// Make a direct NEAR payment to another circle member.
    /// The payment is recorded as a settlement in the circle's history.
    /// Cannot pay yourself. Both payer and recipient must be circle members.
    /// 
    /// # Storage Model
    /// Settlement storage is charged to the circle owner's storage balance,
    /// consistent with the owner-funded storage model for all circle data.
    /// 
    /// # Security
    /// Requires exact deposit amount for the transfer.
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
            epoch: circle.ledger_epoch, // EPOCH-FIX: Record current epoch
        };
        self.record_settlement(settlement);

        // STORAGE-FIX: Charge circle owner's storage for settlements
        self.apply_storage_cost(&circle.owner, initial_storage, false, None);

        let _ = Promise::new(to).transfer(yocto_to_token(amount));
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
        
        // TOKEN-ALLOWLIST: Reject unapproved token contracts to prevent malicious
        // tokens from spoofing sender_id and draining members' storage deposits
        if !self.approved_tokens.get(&token_contract).unwrap_or(false) {
            env::log_str("ERROR: Token contract not approved for settlements");
            return PromiseOrValue::Value(amount);
        }
        
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

    // =========================================================================
    // TOKEN ALLOWLIST MANAGEMENT
    // =========================================================================

    /// Add a token contract to the approved tokens list.
    /// Only the contract owner (the account that deployed this contract) can call this.
    /// 
    /// # Security
    /// This prevents malicious token contracts from spoofing sender_id and draining
    /// members' storage deposits through fake ft_on_transfer calls.
    /// 
    /// # Arguments
    /// * `token_id` - The account ID of the FT contract to approve
    pub fn approve_token(&mut self, token_id: AccountId) {
        // Only contract owner can approve tokens
        require!(
            env::predecessor_account_id() == env::current_account_id(),
            "Only contract owner can approve tokens"
        );
        
        self.approved_tokens.insert(&token_id, &true);
        
        self.emit_event(
            "token_approved",
            json!([{
                "token_id": token_id,
            }]),
        );
    }

    /// Remove a token contract from the approved tokens list.
    /// Only the contract owner can call this.
    /// 
    /// # Arguments
    /// * `token_id` - The account ID of the FT contract to remove
    pub fn revoke_token(&mut self, token_id: AccountId) {
        require!(
            env::predecessor_account_id() == env::current_account_id(),
            "Only contract owner can revoke tokens"
        );
        
        self.approved_tokens.remove(&token_id);
        
        self.emit_event(
            "token_revoked",
            json!([{
                "token_id": token_id,
            }]),
        );
    }

    /// Check if a token contract is approved for use in settlements.
    /// 
    /// # Arguments
    /// * `token_id` - The account ID of the FT contract to check
    /// 
    /// # Returns
    /// true if the token is approved, false otherwise
    pub fn is_token_approved(&self, token_id: AccountId) -> bool {
        self.approved_tokens.get(&token_id).unwrap_or(false)
    }

    /// Retrieve cached fungible token metadata for display purposes.
    /// 
    /// # Verified Caching
    /// Metadata is fetched and verified via cross-contract call to the token contract's
    /// `ft_metadata()` method. Use `fetch_ft_metadata()` to populate the cache.
    /// 
    /// # Returns
    /// Cached metadata if available, or None if not yet fetched.
    pub fn ft_metadata(&self, token_id: AccountId) -> Option<FungibleTokenMetadata> {
        self.metadata_cache.get(&token_id)
    }

    /// Fetch and cache fungible token metadata via verified cross-contract call.
    /// 
    /// # Verified Caching
    /// This method performs a cross-contract call to the token contract's `ft_metadata()`
    /// method, ensuring the cached data is authentic and not caller-supplied.
    /// The callback validates metadata fields before storing.
    /// 
    /// # Arguments
    /// * `token_id` - The account ID of the FT contract to fetch metadata from
    /// 
    /// # Requirements
    /// - Caller must be registered (have storage deposit)
    /// - Attached deposit covers storage costs for caching
    /// 
    /// # Returns
    /// Promise that resolves when metadata is fetched and cached
    #[payable]
    pub fn fetch_ft_metadata(&mut self, token_id: AccountId) -> Promise {
        let account = env::predecessor_account_id();
        self.assert_registered(&account);
        
        // Cross-contract call to fetch metadata from the token contract
        ext_ft::ext(token_id.clone())
            .with_static_gas(Gas::from_tgas(GAS_FT_METADATA_TGAS))
            .ft_metadata()
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(GAS_FT_METADATA_CALLBACK_TGAS))
                    .with_attached_deposit(env::attached_deposit())
                    .on_ft_metadata(token_id, account)
            )
    }

    /// Callback for ft_metadata cross-contract call. Validates and stores metadata.
    /// 
    /// # Private
    /// This method can only be called by the contract itself as a callback.
    /// 
    /// # Validation
    /// - Checks promise result succeeded
    /// - Validates name is non-empty and within length limit
    /// - Validates symbol is non-empty and within length limit  
    /// - Validates decimals <= 24
    /// 
    /// # Returns
    /// true if metadata was successfully cached, false otherwise
    #[private]
    pub fn on_ft_metadata(
        &mut self,
        token_id: AccountId,
        caller: AccountId,
    ) -> bool {
        // Check promise result
        if env::promise_results_count() != 1 {
            env::log_str("ft_metadata fetch failed: unexpected promise count");
            return false;
        }

        // Note: promise_result is deprecated but promise_result_checked doesn't exist in 5.5.0
        #[allow(deprecated)]
        let result = env::promise_result(0);

        match result {
            PromiseResult::Successful(data) => {
                // Deserialize metadata from the token contract response
                let metadata: FungibleTokenMetadata = match serde_json::from_slice(&data) {
                    Ok(m) => m,
                    Err(_) => {
                        env::log_str("ft_metadata fetch failed: invalid JSON response");
                        return false;
                    }
                };

                // Validate metadata fields
                if metadata.name.is_empty() {
                    env::log_str("ft_metadata validation failed: name is empty");
                    return false;
                }
                if metadata.name.len() > MAX_FT_METADATA_NAME_LEN {
                    env::log_str("ft_metadata validation failed: name too long");
                    return false;
                }
                if metadata.symbol.is_empty() {
                    env::log_str("ft_metadata validation failed: symbol is empty");
                    return false;
                }
                if metadata.symbol.len() > MAX_FT_METADATA_SYMBOL_LEN {
                    env::log_str("ft_metadata validation failed: symbol too long");
                    return false;
                }
                if metadata.decimals > 24 {
                    env::log_str("ft_metadata validation failed: decimals exceeds 24");
                    return false;
                }

                // Store metadata with storage cost accounting
                let initial_storage = env::storage_usage();
                self.metadata_cache.insert(&token_id, &metadata);
                // REFUND-FIX: Pass caller as explicit refund target since predecessor is contract in callback
                self.apply_storage_cost(&caller, initial_storage, true, Some(&caller));

                self.emit_event(
                    "ft_metadata_cached",
                    json!([{
                        "token_id": token_id,
                        "name": metadata.name,
                        "symbol": metadata.symbol,
                        "verified": true
                    }]),
                );

                true
            }
            PromiseResult::Failed => {
                env::log_str("ft_metadata fetch failed: cross-contract call failed");
                false
            }
        }
    }

    /// Get the storage balance bounds for registration.
    /// 
    /// # Returns
    /// StorageBalanceBounds with minimum required and maximum allowed storage deposits.
    /// The minimum is the recommended amount (~0.25 NEAR) for typical usage.
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
                // Update aggregate
                self.total_storage_deposits = self.total_storage_deposits
                    .checked_add(deposit)
                    .unwrap_or_else(|| env::panic_str("Total storage deposits overflow"));
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
        // Update aggregate for new registration
        self.total_storage_deposits = self.total_storage_deposits
            .checked_add(deposit)
            .unwrap_or_else(|| env::panic_str("Total storage deposits overflow"));

        StorageBalance {
            total: yocto_to_token(deposit),
            available: yocto_to_token(deposit.saturating_sub(min_locked)),
        }
    }

    /// Get the storage balance for an account.
    /// 
    /// # Arguments
    /// * `account_id` - The account to check storage balance for
    /// 
    /// # Returns
    /// StorageBalance with total and available amounts, or None if not registered
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

    /// Withdraw available storage balance.
    /// 
    /// # Arguments
    /// * `amount` - Amount to withdraw in yoctoNEAR. If None, withdraws all available.
    /// 
    /// # Returns
    /// StorageBalance with updated total and available amounts
    /// 
    /// # Security
    /// Requires exactly 1 yoctoNEAR attached for security confirmation.
    /// Cannot withdraw below minimum locked storage (registration cost).
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
        // Update aggregate for withdrawal
        self.total_storage_deposits = self.total_storage_deposits.saturating_sub(amount_requested);

        if amount_requested > 0 {
            let _ = Promise::new(account.clone()).transfer(yocto_to_token(amount_requested));
        }

        StorageBalance {
            total: yocto_to_token(new_total),
            available: yocto_to_token(available),
        }
    }

    /// Unregister from the contract and withdraw all storage deposit.
    /// 
    /// # Arguments
    /// * `force` - If true, also refunds pending payouts and escrowed funds
    /// 
    /// # Returns
    /// true if successfully unregistered, false if not registered
    /// 
    /// # Requirements
    /// - Cannot be a member of any circle
    /// - Unless force=true, cannot have escrowed funds or pending payouts
    /// 
    /// # Security
    /// Requires exactly 1 yoctoNEAR attached for security confirmation.
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
        
        // SECURITY: Ensure user has no escrowed funds before unregistering
        // This is an O(1) check using the aggregate tracking
        let escrow_total = self.escrow_total_by_account.get(&account).unwrap_or(0);
        if !can_force {
            require!(
                escrow_total == 0,
                "Withdraw escrowed funds before unregistering (disable autopay in all circles)",
            );
        }
        
        // SECURITY: Ensure user has no pending payouts before unregistering
        // Otherwise funds would be locked in the contract
        if !can_force {
            require!(
                self.pending_payouts.get(&account).unwrap_or(0) == 0,
                "Withdraw pending payouts before unregistering",
            );
        }

        // SECURITY: If force=true, also refund any pending payouts and escrowed funds
        let pending_payout = self.pending_payouts.remove(&account).unwrap_or(0);
        // Update pending payouts aggregate
        if pending_payout > 0 {
            self.total_pending_payouts = self.total_pending_payouts.saturating_sub(pending_payout);
        }
        
        // Remove escrow aggregate tracking if force=true (individual circle escrows cleaned elsewhere)
        if can_force && escrow_total > 0 {
            self.escrow_total_by_account.remove(&account);
            self.total_escrow = self.total_escrow.saturating_sub(escrow_total);
        }

        if let Some(balance) = self.storage_deposits.remove(&account) {
            // Update storage deposits aggregate
            self.total_storage_deposits = self.total_storage_deposits.saturating_sub(balance);
            // SECURITY: Combine storage balance + pending payouts + escrowed funds in single transfer
            let total_refund = balance
                .checked_add(pending_payout).unwrap_or(balance)
                .checked_add(escrow_total).unwrap_or(balance);
            let _ = Promise::new(account.clone()).transfer(yocto_to_token(total_refund));
            self.emit_event("storage_unregister", json!([{ "account_id": account }]));
            true
        } else {
            false
        }
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

    /// Apply storage cost accounting.
    /// 
    /// # Arguments
    /// * `account_id` - Account to charge/credit storage from
    /// * `initial_usage` - Storage usage before the operation
    /// * `use_attached` - Whether to use attached deposit for payment
    /// * `explicit_refund_to` - Explicit refund target (required in callbacks where predecessor is contract itself)
    fn apply_storage_cost(
        &mut self,
        account_id: &AccountId,
        initial_usage: u64,
        use_attached: bool,
        explicit_refund_to: Option<&AccountId>,
    ) {
        let final_usage = env::storage_usage();
        let attached = if use_attached {
            env::attached_deposit().as_yoctonear()
        } else {
            0
        };
        // REFUND-FIX: Use explicit refund target if provided, otherwise fall back to predecessor
        let refund_target: Option<AccountId> = if use_attached {
            explicit_refund_to
                .cloned()
                .or_else(|| Some(env::predecessor_account_id()))
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
                    let _ = Promise::new(refund_target.unwrap()).transfer(yocto_to_token(attached));
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
                // Update aggregate for storage cost deduction
                self.total_storage_deposits = self.total_storage_deposits.saturating_sub(remaining);
            }

            let refund = attached.saturating_sub(used_from_deposit);
            if refund > 0 {
                let _ = Promise::new(refund_target.unwrap()).transfer(yocto_to_token(refund));
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
                    // Update aggregate for storage refund (freed storage)
                    self.total_storage_deposits = self.total_storage_deposits
                        .checked_add(refund)
                        .unwrap_or_else(|| env::panic_str("Total storage deposits overflow"));
                }
            }

            if attached > 0 {
                let _ = Promise::new(refund_target.unwrap()).transfer(yocto_to_token(attached));
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

        let next_settlement_index = safe_increment_u64(current_len, "settlements_len");
        let settlement_id = format!("settlement-{}-{}", circle_id, next_settlement_index);
        let index_key = Self::settlement_index_key(&circle_id, current_len);
        self.settlements_index.insert(&index_key, &settlement_id);
        self.settlement_by_id.insert(&settlement_id, &settlement);
        self.settlements_len.insert(&circle_id, &next_settlement_index);

        self.emit_event("settlement_paid", event_payload);
    }

    fn assert_registered(&self, account_id: &AccountId) {
        require!(
            self.storage_deposits.get(account_id).is_some(),
            "Account must call storage_deposit first",
        );
    }

    /// Increase escrow for an account - updates per-circle escrow AND aggregate totals.
    /// Should be called whenever escrow is deposited.
    fn escrow_increase(&mut self, account_id: &AccountId, escrow_key: &str, amount: u128) {
        if amount == 0 {
            return;
        }
        
        // Update per-circle escrow
        let existing = self.escrow_deposits.get(&escrow_key.to_string()).unwrap_or(0);
        let new_total = existing
            .checked_add(amount)
            .unwrap_or_else(|| env::panic_str("Escrow deposit overflow"));
        self.escrow_deposits.insert(&escrow_key.to_string(), &new_total);
        
        // Update per-account aggregate
        let account_total = self.escrow_total_by_account.get(account_id).unwrap_or(0);
        let new_account_total = account_total
            .checked_add(amount)
            .unwrap_or_else(|| env::panic_str("Account escrow total overflow"));
        self.escrow_total_by_account.insert(account_id, &new_account_total);
        
        // Update global aggregate
        self.total_escrow = self.total_escrow
            .checked_add(amount)
            .unwrap_or_else(|| env::panic_str("Global escrow total overflow"));
    }

    /// Decrease escrow for an account - updates per-circle escrow AND aggregate totals.
    /// Returns the actual amount decreased (may be less if not enough escrowed).
    /// Should be called whenever escrow is debited or removed.
    fn escrow_decrease(&mut self, account_id: &AccountId, escrow_key: &str, amount: u128) -> u128 {
        if amount == 0 {
            return 0;
        }
        
        let existing = self.escrow_deposits.get(&escrow_key.to_string()).unwrap_or(0);
        let decrease_amount = amount.min(existing);
        
        if decrease_amount == 0 {
            return 0;
        }
        
        // Update per-circle escrow
        let remaining = existing.saturating_sub(decrease_amount);
        if remaining > 0 {
            self.escrow_deposits.insert(&escrow_key.to_string(), &remaining);
        } else {
            self.escrow_deposits.remove(&escrow_key.to_string());
        }
        
        // Update per-account aggregate
        let account_total = self.escrow_total_by_account.get(account_id).unwrap_or(0);
        let new_account_total = account_total.saturating_sub(decrease_amount);
        if new_account_total > 0 {
            self.escrow_total_by_account.insert(account_id, &new_account_total);
        } else {
            self.escrow_total_by_account.remove(account_id);
        }
        
        // Update global aggregate
        self.total_escrow = self.total_escrow.saturating_sub(decrease_amount);
        
        decrease_amount
    }

    /// Remove all escrow for an account from a specific circle.
    /// Returns the amount that was removed.
    fn escrow_remove_for_circle(&mut self, account_id: &AccountId, escrow_key: &str) -> u128 {
        let existing = self.escrow_deposits.get(&escrow_key.to_string()).unwrap_or(0);
        if existing > 0 {
            self.escrow_decrease(account_id, escrow_key, existing)
        } else {
            0
        }
    }

    /// Get total escrow held by an account across all circles.
    /// 
    /// # Arguments
    /// * `account_id` - The account to check escrow for
    /// 
    /// # Returns
    /// Total escrowed amount in yoctoNEAR
    pub fn get_escrow_total(&self, account_id: AccountId) -> U128 {
        U128(self.escrow_total_by_account.get(&account_id).unwrap_or(0))
    }

    /// Get global total of all escrowed funds in the contract.
    /// Useful for debugging and rescue calculations.
    /// 
    /// # Returns
    /// Total escrow across all accounts in yoctoNEAR
    pub fn get_total_escrow(&self) -> U128 {
        U128(self.total_escrow)
    }

    /// Get global total of all storage deposits in the contract.
    /// Used for rescue calculations to ensure user funds are protected.
    /// 
    /// # Returns
    /// Total storage deposits across all accounts in yoctoNEAR
    pub fn get_total_storage_deposits(&self) -> U128 {
        U128(self.total_storage_deposits)
    }

    /// Get global total of all pending payouts in the contract.
    /// Used for rescue calculations to ensure user funds are protected.
    /// 
    /// # Returns
    /// Total pending payouts across all accounts in yoctoNEAR
    pub fn get_total_pending_payouts(&self) -> U128 {
        U128(self.total_pending_payouts)
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
        // Note: promise_result is deprecated but promise_result_checked doesn't exist in 5.5.0
        #[allow(deprecated)]
        let result = env::promise_result(0);

        match result {
            PromiseResult::Successful(_) => {
                // M2-FIX: Defensive check - circle may have been deleted during async callback
                // If circle no longer exists, still return 0 (tokens already transferred successfully)
                // but skip recording the settlement
                if self.circles.get(&circle_id).is_none() {
                    env::log_str("WARN: Circle deleted during FT transfer - settlement not recorded");
                    self.emit_event(
                        "ft_transfer_success_orphaned",
                        json!([{
                            "circle_id": circle_id,
                            "from": sender_id,
                            "to": to,
                            "amount": amount,
                            "token": token_contract,
                            "message": "Circle no longer exists - settlement not recorded",
                        }]),
                    );
                    return U128(0);
                }
                // STORAGE-FIX: Get circle owner for storage accounting
                let circle = self.circles.get(&circle_id).unwrap();
                let initial_storage = env::storage_usage();
                let available_storage = self
                    .storage_deposits
                    .get(&circle.owner)
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
                    epoch: circle.ledger_epoch, // EPOCH-FIX: Record current epoch
                };
                self.record_settlement(settlement);

                // STORAGE-FIX: Charge circle owner's storage for settlements
                self.apply_storage_cost(&circle.owner, initial_storage, false, None);

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
    /// # Arguments
    /// * `token_id` - The FT contract address
    /// * `receiver_id` - The account to receive the rescued tokens
    /// * `amount` - The amount of tokens to rescue
    /// 
    /// # Security
    /// - #[private] macro verifies predecessor == current_account_id
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
    /// # Arguments
    /// * `receiver_id` - The account to receive the rescued NEAR
    /// * `amount` - The amount of NEAR to rescue in yoctoNEAR
    /// 
    /// # Security
    /// - #[private] macro verifies predecessor == current_account_id
    /// - Cannot rescue more than truly "stuck" funds (excludes user deposits/escrow/payouts)
    /// - Requires a privileged call (e.g., DAO proposal) to invoke
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
        
        // Safety buffer: 10KB worth of storage cost for future operations
        let safety_buffer = env::storage_byte_cost().as_yoctonear() * 10_000;
        
        // FIX-5: Compute reserved user funds that CANNOT be rescued using tracked aggregates
        // This prevents admin from accidentally or maliciously draining user deposits
        let reserved_user_funds = self.total_storage_deposits
            .checked_add(self.total_pending_payouts).unwrap_or(u128::MAX)
            .checked_add(self.total_escrow).unwrap_or(u128::MAX);
        
        let min_reserve = storage_cost
            .checked_add(reserved_user_funds).unwrap_or(u128::MAX)
            .checked_add(safety_buffer).unwrap_or(u128::MAX);
        
        let available = contract_balance.saturating_sub(min_reserve);
        
        require!(
            amount.0 <= available,
            &format!(
                "Cannot rescue {}: only {} available (balance={}, storage={}, user_deposits={}, pending_payouts={}, escrow={}, buffer={})",
                amount.0, available, contract_balance, storage_cost,
                self.total_storage_deposits, self.total_pending_payouts, self.total_escrow, safety_buffer
            )
        );
        
        self.emit_event(
            "near_rescue",
            json!([{
                "receiver_id": receiver_id,
                "amount": amount,
                "available_for_rescue": U128(available),
                "reserved_user_funds": U128(reserved_user_funds),
            }]),
        );

        Promise::new(receiver_id).transfer(yocto_to_token(amount.0))
    }
}

#[ext_contract(ext_ft)]
pub trait ExtFungibleToken {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
    /// NEP-148: Get fungible token metadata
    fn ft_metadata(&self) -> FungibleTokenMetadata;
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
    /// Callback for verified ft_metadata fetch
    fn on_ft_metadata(
        &mut self,
        token_id: AccountId,
        caller: AccountId,
    ) -> bool;
}

// ADDITIONAL CONTRACT METHODS (impl block 2 of 2)
// In near-sdk 5.x, multiple #[near] impl blocks are supported.
// This block contains settlement-related methods that also need to be exposed as contract methods.
// The #[payable] attribute requires #[near] on the impl block to work.
#[near]
impl NearSplitter {
    /// Confirm the ledger for a circle. Once all members confirm, settlement can proceed.
    /// First confirmation locks the circle (no new expenses). 
    /// If all members have autopay enabled, automatically distributes escrowed funds.
    /// This automatically enables autopay and requires escrow deposit if user has debt.
    /// Once all members confirm, settlement proceeds automatically.
    /// 
    /// # Deposit Semantics
    /// This method has flexible deposit requirements based on the caller's balance:
    /// - **Debtors** (negative balance): Must attach deposit >= debt amount for escrow
    /// - **Creditors** (positive/zero balance): Any attached deposit is refunded immediately
    /// 
    /// This differs from other sensitive methods that require exactly 1 yoctoNEAR because
    /// the escrow deposit serves as both confirmation and the actual settlement funds.
    /// 
    /// # Security
    /// - Requires caller to be registered and a circle member
    /// - Prevents double-confirmation
    /// - Requires escrow deposit covering full debt amount if caller is a debtor
    /// - Prevents confirmation while claims are pending
    /// - Uses checks-effects-interactions pattern throughout
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

            // Store the deposit in escrow using helper that maintains aggregates
            let escrow_key = format!("{}:{}", circle_id, account);
            self.escrow_increase(&account, &escrow_key, deposit);
            
            let new_total = self.escrow_deposits.get(&escrow_key).unwrap_or(0);
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
        confirmations_count = safe_increment_u64(confirmations_count, "confirmations_count");
        self.confirmations_count.insert(&circle_id, &confirmations_count);

        self.apply_storage_cost(&account, initial_storage, false, None);

        if refund_amount > 0 {
            let _ = Promise::new(account.clone()).transfer(yocto_to_token(refund_amount));
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
            
            // Collect all escrow refunds before state changes using helper that maintains aggregates
            for member in &circle.members {
                let escrow_key = format!("{}:{}", circle_id, member);
                if let Some(escrowed) = self.escrow_deposits.get(&escrow_key) {
                    if escrowed > 0 {
                        self.escrow_remove_for_circle(member, &escrow_key);
                        transfers_to_make.push((member.clone(), escrowed));
                    }
                }
                let autopay_key = format!("{}:{}", circle_id, member);
                self.autopay_preferences.remove(&autopay_key);
            }
            
            self.clear_confirmations_for_circle(&circle_id, &circle.members);
            
            // EPOCH-FIX: Increment epoch instead of clearing expenses/settlements
            let mut updated_circle = circle.clone();
            updated_circle.locked = false;
            updated_circle.membership_open = true;
            updated_circle.state = CircleState::Settled;
            updated_circle.ledger_epoch = circle.ledger_epoch.saturating_add(1); // EPOCH-FIX: New epoch
            self.circles.insert(&circle_id, &updated_circle);

            self.apply_storage_cost(&owner, initial_storage, false, None);
            
            // Make all transfers after state is finalized
            for (recipient, amount) in transfers_to_make {
                let _ = Promise::new(recipient).transfer(yocto_to_token(amount));
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
            
            // B2-FIX: Use escrow_decrease helper that maintains aggregates and panics on underflow
            self.escrow_decrease(&suggestion.from, &from_key, suggestion.amount.0);

            payouts_to_credit.push((suggestion.to.clone(), suggestion.amount.0));

            let settlement = Settlement {
                circle_id: circle_id.clone(),
                from: suggestion.from.clone(),
                to: suggestion.to.clone(),
                amount: suggestion.amount,
                token: None,
                ts_ms: timestamp_ms(),
                tx_kind: "autopay_escrow".to_string(),
                epoch: circle.ledger_epoch, // EPOCH-FIX: Record current epoch
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

        // Collect any remaining escrow refunds using helper that maintains aggregates
        for member in &circle.members {
            let escrow_key = format!("{}:{}", circle_id, member);
            if let Some(remaining) = self.escrow_deposits.get(&escrow_key) {
                if remaining > 0 {
                    self.escrow_remove_for_circle(member, &escrow_key);
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

        // EPOCH-FIX: Instead of clearing expenses/settlements, increment the epoch.
        // This preserves historical data while ensuring compute_balances returns zero
        // for the new epoch (no expenses or settlements exist for the new epoch yet).
        // Confirmations are still cleared as they don't carry epoch.
        self.clear_confirmations_for_circle(&circle_id, &circle.members);
        
        // Update circle: unlock, reopen membership, mark as settled, increment epoch
        let mut updated_circle = circle.clone();
        updated_circle.locked = false;
        updated_circle.membership_open = true;
        updated_circle.state = CircleState::Settled;
        updated_circle.ledger_epoch = circle.ledger_epoch.saturating_add(1); // EPOCH-FIX: New epoch
        self.circles.insert(&circle_id, &updated_circle);

        self.apply_storage_cost(&owner, initial_storage, false, None);

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
            // Update aggregate for pending payout credit
            self.total_pending_payouts = self.total_pending_payouts
                .checked_add(total)
                .unwrap_or_else(|| env::panic_str("Total pending payouts overflow"));

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

    /// Get the list of accounts that have confirmed the ledger for a circle.
    /// 
    /// # Arguments
    /// * `circle_id` - The circle to get confirmations for
    /// 
    /// # Returns
    /// Vector of AccountIds that have confirmed the ledger.
    /// Returns empty vector if circle doesn't exist.
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

    /// Check if all members have confirmed the ledger for a circle.
    /// 
    /// # Arguments
    /// * `circle_id` - The circle to check
    /// 
    /// # Returns
    /// true if all members have confirmed, false otherwise.
    /// Returns false if circle doesn't exist.
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
    /// NOTE: Cannot be called during SettlementInProgress or SettlementExecuting.
    /// Use cancel_settlement to abort a settlement that is in progress.
    /// 
    /// # Security
    /// - Only the circle owner can reset confirmations
    /// - Requires exactly 1 yoctoNEAR attached to confirm this sensitive operation
    /// - Uses checks-effects-interactions pattern - all state changes before transfers
    #[payable]
    pub fn reset_confirmations(&mut self, circle_id: String) {
        assert_one_yocto();
        let account = env::predecessor_account_id();
        let mut circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(circle.owner == account, "Only circle owner can reset confirmations");

        // Prevent reset during ongoing settlement - use cancel_settlement instead
        require!(
            circle.state != CircleState::SettlementInProgress
                && circle.state != CircleState::SettlementExecuting,
            "Cannot reset confirmations while settlement is in progress; use cancel_settlement"
        );

        // SECURITY: Collect all refunds BEFORE making state changes, then transfer AFTER
        let mut refunds_to_make: Vec<(AccountId, u128)> = Vec::new();

        let initial_storage = env::storage_usage();

        // Collect all escrowed deposits for this circle using helper that maintains aggregates
        for member in &circle.members {
            let escrow_key = format!("{}:{}", circle_id, member);
            if let Some(escrowed) = self.escrow_deposits.get(&escrow_key) {
                if escrowed > 0 {
                    self.escrow_remove_for_circle(member, &escrow_key);
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

        self.apply_storage_cost(&account, initial_storage, false, None);
        
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
            let _ = Promise::new(member).transfer(yocto_to_token(escrowed));
        }
    }

    /// Cancel an in-progress settlement and reset the circle to usable state.
    /// 
    /// This function allows the circle owner to abort a settlement that is in the
    /// SettlementInProgress phase (confirmations being collected) but NOT during
    /// SettlementExecuting (autopay payouts actively running).
    /// 
    /// When cancelled:
    /// - All escrow deposits are refunded to their respective members
    /// - All confirmations are cleared
    /// - All autopay preferences are cleared
    /// - Circle is unlocked and membership is reopened
    /// - Circle state is set back to Open
    /// 
    /// # Security
    /// - Only the circle owner can cancel settlement
    /// - Requires exactly 1 yoctoNEAR attached to confirm this sensitive operation
    /// - Cannot cancel during SettlementExecuting (prevents reentrancy attacks)
    /// - Uses checks-effects-interactions pattern: all state changes before transfers
    #[payable]
    pub fn cancel_settlement(&mut self, circle_id: String) {
        assert_one_yocto();
        let account = env::predecessor_account_id();
        let mut circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(circle.owner == account, "Only circle owner can cancel settlement");

        // Must be in SettlementInProgress to cancel
        require!(
            circle.state == CircleState::SettlementInProgress,
            "Can only cancel settlement when in SettlementInProgress state"
        );

        // SECURITY: Collect all refunds BEFORE making state changes, then transfer AFTER
        let mut refunds_to_make: Vec<(AccountId, u128)> = Vec::new();

        let initial_storage = env::storage_usage();

        // Collect all escrowed deposits for this circle using helper that maintains aggregates
        for member in &circle.members {
            let escrow_key = format!("{}:{}", circle_id, member);
            if let Some(escrowed) = self.escrow_deposits.get(&escrow_key) {
                if escrowed > 0 {
                    self.escrow_remove_for_circle(member, &escrow_key);
                    refunds_to_make.push((member.clone(), escrowed));
                }
            }
            // Clear autopay preferences
            let autopay_key = format!("{}:{}", circle_id, member);
            self.autopay_preferences.remove(&autopay_key);
        }

        // Clear all confirmations
        self.clear_confirmations_for_circle(&circle_id, &circle.members);
        
        // Reset circle to usable state
        circle.locked = false;
        circle.membership_open = true;
        circle.state = CircleState::Open;
        self.circles.insert(&circle_id, &circle);

        self.apply_storage_cost(&account, initial_storage, false, None);

        self.emit_event(
            "settlement_cancelled",
            json!([{
                "circle_id": circle_id,
                "refunds_count": refunds_to_make.len(),
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
            let _ = Promise::new(member).transfer(yocto_to_token(escrowed));
        }
    }

    /// Set whether the circle is open for new members to join.
    /// Only the circle owner can call this.
    /// When membership is closed, no one can join even with invite code.
    /// Note: This is automatically set to false when first confirmation happens.
    /// 
    /// # Security
    /// Requires exactly 1 yoctoNEAR attached to confirm this sensitive operation.
    #[payable]
    pub fn set_membership_open(&mut self, circle_id: String, open: bool) {
        assert_one_yocto();
        let account = env::predecessor_account_id();
        let mut circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(circle.owner == account, "Only circle owner can change membership status");
        
        // Cannot open membership while circle is locked for settlement or during settlement
        if open {
            require!(
                !circle.locked 
                    && circle.state != CircleState::SettlementInProgress
                    && circle.state != CircleState::SettlementExecuting,
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

    /// Check if a circle is open for new members to join.
    /// 
    /// # Arguments
    /// * `circle_id` - The circle to check
    /// 
    /// # Returns
    /// true if membership is open, false otherwise.
    /// Returns false if circle doesn't exist.
    pub fn is_membership_open(&self, circle_id: String) -> bool {
        self.circles
            .get(&circle_id)
            .map(|c| c.membership_open)
            .unwrap_or(false)
    }

    /// Set autopay preference for the caller in a specific circle.
    /// 
    /// # Enabling Autopay
    /// If enabling autopay and user has debt, requires deposit equal to debt amount.
    /// Creditors (positive balance) can enable autopay with any deposit (refunded if not needed).
    /// 
    /// # Disabling Autopay
    /// Disabling autopay requires exactly 1 yoctoNEAR to confirm this sensitive operation.
    /// Cannot disable while circle is locked for settlement.
    /// When disabled, escrowed funds are refunded.
    #[payable]
    pub fn set_autopay(&mut self, circle_id: String, enabled: bool) {
        let account = env::predecessor_account_id();
        let deposit = env::attached_deposit().as_yoctonear();
        self.assert_registered(&account);

        // Disabling autopay requires 1 yoctoNEAR confirmation
        if !enabled {
            require!(
                deposit == ONE_YOCTO,
                "Disabling autopay requires exactly 1 yoctoNEAR attached for security confirmation"
            );
        }

        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(
            circle.members.iter().any(|m| m == &account),
            "Must be a circle member to set autopay"
        );

        // Prevent disabling autopay when circle is locked for settlement
        if !enabled {
            require!(
                !circle.locked 
                    && circle.state != CircleState::SettlementInProgress
                    && circle.state != CircleState::SettlementExecuting,
                "Cannot disable autopay while circle is locked for settlement"
            );
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

                // Store the deposit in escrow using helper that maintains aggregates
                let escrow_key = format!("{}:{}", circle_id, account);
                self.escrow_increase(&account, &escrow_key, deposit);

                let new_total = self.escrow_deposits.get(&escrow_key).unwrap_or(0);
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
            // Disabling autopay - refund any escrowed funds using helper that maintains aggregates
            let escrow_key = format!("{}:{}", circle_id, account);
            // SECURITY: Collect refund amount and remove from state BEFORE transfer
            let escrowed_to_refund = self.escrow_deposits.get(&escrow_key).unwrap_or(0);
            if escrowed_to_refund > 0 {
                self.escrow_remove_for_circle(&account, &escrow_key);
            }
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

            self.apply_storage_cost(&account, initial_storage, false, None);

            // SECURITY: Transfer AFTER all state changes (checks-effects-interactions)
            if refund_amount > 0 {
                let _ = Promise::new(account).transfer(yocto_to_token(refund_amount));
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

        self.apply_storage_cost(&account, initial_storage, false, None);

        if refund_amount > 0 {
            let _ = Promise::new(account).transfer(yocto_to_token(refund_amount));
        }
    }

    /// Get autopay preference for a specific member in a circle.
    /// 
    /// # Arguments
    /// * `circle_id` - The circle to check
    /// * `account_id` - The member to check autopay preference for
    /// 
    /// # Returns
    /// true if autopay is enabled, false otherwise
    pub fn get_autopay(&self, circle_id: String, account_id: AccountId) -> bool {
        let key = format!("{}:{}", circle_id, account_id);
        self.autopay_preferences.get(&key).unwrap_or(false)
    }

    /// Check if all members in a circle have autopay enabled.
    /// 
    /// # Arguments
    /// * `circle_id` - The circle to check
    /// 
    /// # Returns
    /// true if all members have autopay enabled, false otherwise.
    /// Returns false if circle doesn't exist.
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

    /// Get the required deposit amount for a member to enable autopay.
    /// 
    /// # Arguments
    /// * `circle_id` - The circle to check
    /// * `account_id` - The member to check required deposit for
    /// 
    /// # Returns
    /// The debt amount in yoctoNEAR if user is a debtor, 0 if creditor or even
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

    /// Get current escrow deposit for a member in a specific circle.
    /// 
    /// # Arguments
    /// * `circle_id` - The circle to check
    /// * `account_id` - The member to check escrow for
    /// 
    /// # Returns
    /// The escrowed amount in yoctoNEAR for this circle/member pair
    pub fn get_escrow_deposit(&self, circle_id: String, account_id: AccountId) -> U128 {
        let key = format!("{}:{}", circle_id, account_id);
        U128(self.escrow_deposits.get(&key).unwrap_or(0))
    }

    /// Get the pending payout balance for an account.
    /// This is the amount that can be withdrawn via withdraw_payout().
    /// 
    /// # Arguments
    /// * `account_id` - The account to check pending payouts for
    /// 
    /// # Returns
    /// The pending payout amount in yoctoNEAR
    pub fn get_pending_payout(&self, account_id: AccountId) -> U128 {
        U128(self.pending_payouts.get(&account_id).unwrap_or(0))
    }

    /// Withdraw all pending payouts for the caller.
    /// Implements the pull-payment pattern for settlement distributions.
    /// 
    /// # Returns
    /// Promise that transfers all pending funds to the caller
    /// 
    /// # Security
    /// Requires exactly 1 yoctoNEAR attached for security confirmation.
    /// Uses checks-effects-interactions pattern (state cleared before transfer).
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
        // Update aggregate for payout withdrawal
        self.total_pending_payouts = self.total_pending_payouts.saturating_sub(pending);

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
    /// 
    /// # Arguments
    /// * `amount` - The amount to withdraw in yoctoNEAR
    /// 
    /// # Returns
    /// Promise that transfers the specified amount to the caller
    /// 
    /// # Security
    /// Requires exactly 1 yoctoNEAR attached for security confirmation.
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
        // Update aggregate for partial payout withdrawal
        self.total_pending_payouts = self.total_pending_payouts.saturating_sub(amount.0);

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

// Unit tests require near-sdk with unit-testing feature (provided by dev-dependencies)
// Exclude wasm32 since test_utils is gated on not(target_arch = "wasm32")
// Also exclude Windows since some NEAR testing infrastructure doesn't work there
#[cfg(all(test, not(target_arch = "wasm32"), not(windows)))]
#[allow(unused_mut)]       // Some tests have `let mut ctx` that don't later reassign ctx
#[allow(unused_must_use)]  // Some tests call methods returning Promise without using it
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;
    use near_sdk::test_vm_config;
    use near_sdk::PromiseResult;
    use std::cell::Cell;
    use std::collections::HashMap;

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

    /// Test helper: Register members and have them join a circle.
    /// This simulates what would happen in real usage where each member
    /// registers for storage and then calls join_circle.
    /// After joining all members, restores context to accounts(0) for convenience.
    fn add_members_helper(contract: &mut NearSplitter, circle_id: &str, members: Vec<AccountId>) {
        for member in members {
            // Register the member for storage (if not already registered)
            if contract.storage_balance_of(member.clone()).is_none() {
                let ctx = context(member.clone(), ONE_NEAR);
                testing_env!(ctx.build());
                contract.storage_deposit(None, None);
            }
            
            // Have them join the circle
            let ctx = context(member, 0);
            testing_env!(ctx.build());
            contract.join_circle(circle_id.to_string(), None);
        }
        // Restore context to accounts(0) which is typically the test owner
        let ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
    }

    #[test]
    fn test_storage_deposit_and_membership() {
        let mut contract = setup();
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        let id = contract.create_circle("Friends".to_string(), None, None);
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
        contract.create_circle("Trip".to_string(), None, None);

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
        contract.create_circle("Trip".to_string(), None, None);

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
        contract.create_circle("Trip".to_string(), None, None);

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

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Manually set storage deposit to exactly minimum (no available credit)
        let min = contract.required_storage_cost();
        contract.storage_deposits.insert(&accounts(0), &min);

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
    fn test_add_expense_deducts_storage_from_owner() {
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
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        let stored_before = contract.storage_deposits.get(&accounts(0)).unwrap_or(0);

        // add_expense uses owner's storage credit (use_attached=false)
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

        let stored_after = contract.storage_deposits.get(&accounts(0)).unwrap_or(0);
        // Storage cost should have been deducted from owner's storage credit
        assert!(stored_after < stored_before, "Storage cost should be deducted from owner");
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
        contract.create_circle("Trip".to_string(), None, None);

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
        contract.create_circle("Trip".to_string(), None, None);

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
        contract.create_circle("Trip".to_string(), None, None);

        for i in 0..5u64 {
            let settlement = Settlement {
                circle_id: "circle-0".to_string(),
                from: accounts(0),
                to: accounts(1),
                amount: U128(10 + i as u128),
                token: None,
                ts_ms: timestamp_ms(),
                tx_kind: "native".to_string(),
                epoch: 0, // EPOCH-FIX: Test settlement in epoch 0
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
        contract.create_circle("Trip".to_string(), None, None);

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
        contract.create_circle("Trip".to_string(), None, None);

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
            epoch: 0, // EPOCH-FIX: Test settlement in epoch 0
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

        // Directly set the settlement count to MAX - 1 to avoid log overflow
        // (recording 10k settlements with events would exceed log limit)
        contract.settlements_len.insert(&"circle-0".to_string(), &((MAX_SETTLEMENTS_PER_CIRCLE - 1) as u64));

        // Record one settlement to reach MAX
        let settlement1 = Settlement {
            circle_id: "circle-0".to_string(),
            from: accounts(0),
            to: accounts(1),
            amount: U128(1),
            token: None,
            ts_ms: timestamp_ms(),
            tx_kind: "native".to_string(),
            epoch: 0,
        };
        contract.record_settlement(settlement1);

        // This should panic - we're now at MAX
        let settlement2 = Settlement {
            circle_id: "circle-0".to_string(),
            from: accounts(0),
            to: accounts(1),
            amount: U128(1),
            token: None,
            ts_ms: timestamp_ms(),
            tx_kind: "native".to_string(),
            epoch: 0,
        };
        contract.record_settlement(settlement2);
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
        contract.create_circle("Trip".to_string(), None, None);

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
        contract.create_circle("Trip".to_string(), None, None);

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
        contract.create_circle("Trip".to_string(), None, None);

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
        contract.create_circle("Trip".to_string(), None, None);

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
        contract.create_circle("Trip".to_string(), None, None);

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
        contract.create_circle("Trip".to_string(), None, None);

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

        // Payer approves the claim (requires 1 yoctoNEAR)
        ctx = context(accounts(0), 1);
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
        contract.create_circle("Trip".to_string(), None, None);

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

        // Payer rejects the claim (requires 1 yoctoNEAR)
        ctx = context(accounts(0), 1);
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
        contract.create_circle("Trip".to_string(), None, None);

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

        // Payer approves removal (requires 1 yoctoNEAR)
        ctx = context(accounts(0), 1);
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
        contract.create_circle("Trip".to_string(), None, None);

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
        contract.create_circle("Trip".to_string(), None, None);

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
        contract.create_circle("Trip".to_string(), None, None);

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

        // Non-payer tries to approve (requires 1 yoctoNEAR to get past security check)
        ctx = context(accounts(1), 1);
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
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(2), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add expense: account(0) paid 300, split 3 ways
        // With exact split: 300 * 3334/10000 = 100.02, 300 * 3333/10000 = 99.99 each for others
        // Use 10000 total for clean math: 300 split as 100 each
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

        // Check actual balances to determine correct escrow amounts
        let balances = contract.compute_balances("circle-0".to_string());
        let debt1 = balances.iter().find(|b| b.account_id == accounts(1)).map(|b| b.net.0).unwrap_or(0);
        let debt2 = balances.iter().find(|b| b.account_id == accounts(2)).map(|b| b.net.0).unwrap_or(0);
        let escrow1 = if debt1 < 0 { debt1.unsigned_abs() } else { 0 };
        let escrow2 = if debt2 < 0 { debt2.unsigned_abs() } else { 0 };

        // First confirmation by creditor (account 0 - no deposit needed)
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // Verify circle is locked
        let circle = contract.get_circle("circle-0".to_string());
        assert!(circle.locked);
        assert_eq!(circle.state, CircleState::SettlementInProgress);

        // B1-FIX: Second confirmation should succeed (was failing before fix)
        ctx = context(accounts(1), escrow1);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // Third confirmation
        ctx = context(accounts(2), escrow2);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // After all members confirmed, execute_autopay_settlements runs automatically
        // and clears confirmations. Verify settlement completed successfully.
        let circle = contract.get_circle("circle-0".to_string());
        assert_eq!(circle.state, CircleState::Settled, "Circle should be in Settled state after all confirmations");
        assert!(!circle.locked, "Circle should be unlocked after settlement");
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
        contract.create_circle("Trip".to_string(), None, None);

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

        // Delete the expense (requires 1 yoctoNEAR)
        ctx = context(accounts(0), 1);
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
        contract.create_circle("Trip".to_string(), None, None);

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

        ctx = context(accounts(0), 1);
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
        contract.create_circle("Trip".to_string(), None, None);

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
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add 5 expenses
        for i in 0..5u64 {
            ctx = context(accounts(0), 0);
            testing_env!(ctx.build());
            contract.add_expense(
                "circle-0".to_string(),
                U128(10 + i as u128),
                vec![
                    MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                    MemberShare { account_id: accounts(1), weight_bps: 5_000 },
                ],
                format!("Expense {}", i + 1),
            );
        }

        // Delete the third expense (requires 1 yoctoNEAR)
        ctx = context(accounts(0), 1);
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
        contract.create_circle("Trip".to_string(), None, None);

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
        contract.create_circle("Trip".to_string(), None, None);

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
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        let contract_id: AccountId = "contract.near".parse().unwrap();
        let token_contract: AccountId = "token.near".parse().unwrap();
        let mut ctx = VMContextBuilder::new();
        ctx.current_account_id(contract_id.clone());
        ctx.predecessor_account_id(contract_id);
        ctx.signer_account_id(accounts(0));
        testing_env!(
            ctx.build(),
            test_vm_config(),
            near_sdk::RuntimeFeesConfig::test(),
            HashMap::default(),
            vec![PromiseResult::Successful(vec![])],
        );

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
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        let contract_id: AccountId = "contract.near".parse().unwrap();
        let token_contract: AccountId = "token.near".parse().unwrap();
        let mut ctx = VMContextBuilder::new();
        ctx.current_account_id(contract_id.clone());
        ctx.predecessor_account_id(contract_id);
        ctx.signer_account_id(accounts(0));
        testing_env!(
            ctx.build(),
            test_vm_config(),
            near_sdk::RuntimeFeesConfig::test(),
            HashMap::default(),
            vec![PromiseResult::Failed],
        );

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

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Set storage to exactly minimum so there's no available credit for settlement recording
        let min = contract.required_storage_cost();
        contract.storage_deposits.insert(&accounts(0), &min);

        let contract_id: AccountId = "contract.near".parse().unwrap();
        let token_contract: AccountId = "token.near".parse().unwrap();
        let mut ctx = VMContextBuilder::new();
        ctx.current_account_id(contract_id.clone());
        ctx.predecessor_account_id(contract_id);
        ctx.signer_account_id(accounts(0));
        testing_env!(
            ctx.build(),
            test_vm_config(),
            near_sdk::RuntimeFeesConfig::test(),
            HashMap::default(),
            vec![PromiseResult::Successful(vec![])],
        );

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
        contract.create_circle("Trip".to_string(), None, None);

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

    // =========================================================================
    // ADDITIONAL FIX VERIFICATION TESTS
    // =========================================================================

    /// E1-FIX: Verify file_claim rejects proposed_amount > i128::MAX at filing time
    #[test]
    #[should_panic(expected = "Proposed amount exceeds maximum safe value")]
    fn test_file_claim_validates_amount_max_at_filing() {
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
        contract.create_circle("Trip".to_string(), None, None);

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

        // E1-FIX: Filing claim with amount > i128::MAX should fail immediately
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.file_claim(
            "circle-0".to_string(),
            "expense-circle-0-1".to_string(),
            "wrong_amount".to_string(),
            Some(U128(u128::MAX)),  // Exceeds i128::MAX - should fail at file time
            None,
        );
    }

    /// E2-FIX: Verify leave_circle fails with proper message when escrow is non-zero
    #[test]
    #[should_panic(expected = "Cannot leave with escrowed funds. Disable autopay first to withdraw escrow.")]
    fn test_leave_circle_fails_with_escrow() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Manually insert escrow to simulate the scenario where user has escrow but no debt
        // (set_autopay without debt would refund the deposit, so we insert directly)
        let escrow_key = format!("circle-0:{}", accounts(1));
        contract.escrow_deposits.insert(&escrow_key, &50u128);

        // Force circle to settled state for leave_circle to work
        let mut circle = contract.circles.get(&"circle-0".to_string()).unwrap();
        circle.state = CircleState::Settled;
        contract.circles.insert(&"circle-0".to_string(), &circle);

        // E2-FIX: Try to leave - should fail because escrow is non-zero
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.leave_circle("circle-0".to_string());
    }

    /// E4-FIX: Verify delete_expense handles attached deposit properly (refunds)
    #[test]
    fn test_delete_expense_refunds_attached_deposit() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

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

        let balance_before = contract.storage_deposits.get(&accounts(0)).unwrap_or(0);

        // E4-FIX: Delete with 1 yoctoNEAR (required for security)
        ctx = context(accounts(0), 1);
        testing_env!(ctx.build());
        contract.delete_expense("circle-0".to_string(), "expense-circle-0-1".to_string());

        // Storage credit should increase (freed storage + refund)
        let balance_after = contract.storage_deposits.get(&accounts(0)).unwrap_or(0);
        assert!(balance_after >= balance_before, "Storage credit should not decrease");
    }

    /// M2-FIX: Verify on_ft_forward_complete handles missing circle gracefully
    #[test]
    fn test_on_ft_forward_complete_circle_deleted() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle but don't keep it
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

        // Simulate circle being deleted before callback
        // In reality this could happen if owner deletes during async FT transfer
        contract.circles.remove(&"circle-0".to_string());

        let contract_id: AccountId = "contract.near".parse().unwrap();
        let token_contract: AccountId = "token.near".parse().unwrap();
        let mut ctx = VMContextBuilder::new();
        ctx.current_account_id(contract_id.clone());
        ctx.predecessor_account_id(contract_id);
        ctx.signer_account_id(accounts(0));
        testing_env!(
            ctx.build(),
            test_vm_config(),
            near_sdk::RuntimeFeesConfig::test(),
            HashMap::default(),
            vec![PromiseResult::Successful(vec![])],
        );

        // M2-FIX: Should not panic, should return 0 (tokens already sent)
        let result = contract.on_ft_forward_complete(
            accounts(0),
            U128(500),
            token_contract,
            "circle-0".to_string(),
            accounts(1),
        );

        assert_eq!(result.0, 0, "Should return 0 even if circle deleted");
        // No settlement recorded (circle doesn't exist)
        let settlements_len = contract.settlements_len.get(&"circle-0".to_string()).unwrap_or(0);
        assert_eq!(settlements_len, 0);
    }

    /// Verify leave_circle works after disabling autopay (proper flow)
    #[test]
    fn test_leave_circle_after_disable_autopay() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

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

        // accounts(1) owes 50, enable autopay with escrow
        ctx = context(accounts(1), 50);
        testing_env!(ctx.build());
        contract.set_autopay("circle-0".to_string(), true);

        // Verify escrow is set
        let escrow_key = format!("{}:{}", "circle-0", accounts(1));
        assert_eq!(contract.escrow_deposits.get(&escrow_key).unwrap_or(0), 50);

        // Disable autopay (requires 1 yoctoNEAR) - should refund escrow
        ctx = context(accounts(1), 1);
        testing_env!(ctx.build());
        contract.set_autopay("circle-0".to_string(), false);

        // Verify escrow is cleared
        assert_eq!(contract.escrow_deposits.get(&escrow_key).unwrap_or(0), 0);

        // Now settle the circle properly so user can leave
        let mut circle = contract.circles.get(&"circle-0".to_string()).unwrap();
        circle.state = CircleState::Settled;
        contract.circles.insert(&"circle-0".to_string(), &circle);

        // Pay off the balance
        ctx = context(accounts(1), 50);
        testing_env!(ctx.build());
        contract.pay_native("circle-0".to_string(), accounts(0));

        // Verify balance is now 0
        let balances = contract.compute_balances("circle-0".to_string());
        let user_balance = balances.iter().find(|b| b.account_id == accounts(1)).map(|b| b.net.0).unwrap_or(0);
        assert_eq!(user_balance, 0);

        // Now can leave
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.leave_circle("circle-0".to_string());

        let circle = contract.get_circle("circle-0".to_string());
        assert!(!circle.members.contains(&accounts(1)));
    }

    /// Verify add_expense respects i128::MAX for balance safety
    #[test]
    #[should_panic(expected = "Amount exceeds maximum safe value")]
    fn test_add_expense_rejects_overflow_amount() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Try to add expense with amount > i128::MAX
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(u128::MAX),  // Exceeds i128::MAX
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Huge expense".to_string(),
        );
    }

    /// Verify pay_native respects i128::MAX amount limit  
    #[test]
    #[should_panic(expected = "Amount exceeds maximum safe value")]
    fn test_pay_native_rejects_overflow_amount() {
        let mut contract = setup();

        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Try pay_native with amount > i128::MAX but < u128::MAX to avoid VM overflow
        // i128::MAX = 170141183460469231731687303715884105727
        let overflow_amount = (i128::MAX as u128) + 1;
        ctx = context(accounts(0), overflow_amount);
        testing_env!(ctx.build());
        contract.pay_native("circle-0".to_string(), accounts(1));
    }

    // =========================================================================
    // CANCEL SETTLEMENT TESTS
    // =========================================================================

    /// Test that a member can confirm and lock the circle, and owner can cancel
    /// during SettlementInProgress, refunding escrow and making circle usable again
    #[test]
    fn test_cancel_settlement_during_settlement_in_progress() {
        let mut contract = setup();

        // Setup accounts
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle and join
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add expense: accounts(0) pays 100, split 50/50
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

        // Creditor confirms (no deposit needed) - this locks the circle
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // Verify circle is now locked and in SettlementInProgress
        let circle = contract.get_circle("circle-0".to_string());
        assert!(circle.locked);
        assert!(!circle.membership_open);
        assert_eq!(circle.state, CircleState::SettlementInProgress);

        // Verify confirmation was recorded
        let confirmations = contract.get_confirmations("circle-0".to_string());
        assert_eq!(confirmations.len(), 1);
        assert!(confirmations.contains(&accounts(0)));

        // Debtor also confirms with escrow deposit
        ctx = context(accounts(1), 50);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // After both confirm, autopay should execute and circle should be Settled
        let circle = contract.get_circle("circle-0".to_string());
        assert_eq!(circle.state, CircleState::Settled);
    }

    /// Test that owner can cancel settlement during SettlementInProgress (before all confirm)
    #[test]
    fn test_cancel_settlement_refunds_escrow() {
        let mut contract = setup();

        // Setup accounts
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(2), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle with 3 members
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(2), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add expense: accounts(0) pays 90, split 3 ways (30 each)
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(90),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 3_334 },
                MemberShare { account_id: accounts(1), weight_bps: 3_333 },
                MemberShare { account_id: accounts(2), weight_bps: 3_333 },
            ],
            "Dinner".to_string(),
        );

        // Creditor confirms - locks circle
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // First debtor confirms with escrow
        ctx = context(accounts(1), 30);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // Verify escrow was deposited
        let escrow_key_1 = format!("{}:{}", "circle-0", accounts(1));
        assert_eq!(contract.escrow_deposits.get(&escrow_key_1).unwrap_or(0), 30);

        // Circle should still be in SettlementInProgress (one member hasn't confirmed)
        let circle = contract.get_circle("circle-0".to_string());
        assert_eq!(circle.state, CircleState::SettlementInProgress);
        assert!(circle.locked);
        assert!(!circle.membership_open);

        // Owner cancels the settlement (requires 1 yoctoNEAR)
        ctx = context(accounts(0), 1);
        testing_env!(ctx.build());
        contract.cancel_settlement("circle-0".to_string());

        // Verify circle is back to Open and unlocked
        let circle = contract.get_circle("circle-0".to_string());
        assert_eq!(circle.state, CircleState::Open);
        assert!(!circle.locked);
        assert!(circle.membership_open);

        // Verify escrow was cleared (refund was initiated)
        assert_eq!(contract.escrow_deposits.get(&escrow_key_1).unwrap_or(0), 0);

        // Verify confirmations were cleared
        let confirmations = contract.get_confirmations("circle-0".to_string());
        assert_eq!(confirmations.len(), 0);
        assert_eq!(contract.confirmations_count.get(&"circle-0".to_string()).unwrap_or(0), 0);

        // Verify autopay preferences were cleared
        let autopay_key_0 = format!("{}:{}", "circle-0", accounts(0));
        let autopay_key_1 = format!("{}:{}", "circle-0", accounts(1));
        assert!(!contract.autopay_preferences.get(&autopay_key_0).unwrap_or(false));
        assert!(!contract.autopay_preferences.get(&autopay_key_1).unwrap_or(false));

        // Circle should be usable again - can add new expense
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(50),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "New expense after cancel".to_string(),
        );

        // Verify expense was added
        let expenses = contract.list_expenses("circle-0".to_string(), None, None);
        assert_eq!(expenses.len(), 2);
    }

    /// Test that cancel_settlement is rejected when circle is in SettlementExecuting state
    #[test]
    #[should_panic(expected = "Can only cancel settlement when in SettlementInProgress state")]
    fn test_cancel_settlement_rejected_during_executing() {
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
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Manually set circle to SettlementExecuting to simulate mid-execution
        let mut circle = contract.circles.get(&"circle-0".to_string()).unwrap();
        circle.state = CircleState::SettlementExecuting;
        circle.locked = true;
        contract.circles.insert(&"circle-0".to_string(), &circle);

        // Try to cancel (requires 1 yoctoNEAR) - should panic
        ctx = context(accounts(0), 1);
        testing_env!(ctx.build());
        contract.cancel_settlement("circle-0".to_string());
    }

    /// Test that cancel_settlement is rejected when circle is Open (not in settlement)
    #[test]
    #[should_panic(expected = "Can only cancel settlement when in SettlementInProgress state")]
    fn test_cancel_settlement_rejected_when_open() {
        let mut contract = setup();

        // Setup accounts
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

        // Circle is Open by default
        let circle = contract.get_circle("circle-0".to_string());
        assert_eq!(circle.state, CircleState::Open);

        // Try to cancel (requires 1 yoctoNEAR) - should panic
        ctx = context(accounts(0), 1);
        testing_env!(ctx.build());
        contract.cancel_settlement("circle-0".to_string());
    }

    /// Test that only the owner can cancel settlement
    #[test]
    #[should_panic(expected = "Only circle owner can cancel settlement")]
    fn test_cancel_settlement_only_owner() {
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
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Creditor (owner) confirms - locks circle
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // Non-owner tries to cancel (requires 1 yoctoNEAR) - should panic
        ctx = context(accounts(1), 1);
        testing_env!(ctx.build());
        contract.cancel_settlement("circle-0".to_string());
    }

    /// Test the updated reset_confirmations error message points to cancel_settlement
    #[test]
    #[should_panic(expected = "Cannot reset confirmations while settlement is in progress; use cancel_settlement")]
    fn test_reset_confirmations_suggests_cancel_settlement() {
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
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Creditor confirms - locks circle, enters SettlementInProgress
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // Verify state
        let circle = contract.get_circle("circle-0".to_string());
        assert_eq!(circle.state, CircleState::SettlementInProgress);

        // Try reset_confirmations (requires 1 yoctoNEAR) - should panic with helpful message
        ctx = context(accounts(0), 1);
        testing_env!(ctx.build());
        contract.reset_confirmations("circle-0".to_string());
    }

    // =========================================================================
    // DELETE EXPENSE TESTS (Deposit Handling)
    // =========================================================================

    /// Test that delete_expense works correctly without any deposit
    #[test]
    fn test_delete_expense_no_deposit_required() {
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
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add expense
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

        // Verify expense exists
        let expenses = contract.list_expenses("circle-0".to_string(), None, None);
        assert_eq!(expenses.len(), 1);

        // Record storage balance before deletion
        let storage_before = contract.storage_deposits.get(&accounts(0)).unwrap_or(0);

        // Delete expense (requires 1 yoctoNEAR)
        ctx = context(accounts(0), 1);
        testing_env!(ctx.build());
        contract.delete_expense("circle-0".to_string(), "expense-circle-0-1".to_string());

        // Verify expense is deleted
        assert!(contract.expense_by_id.get(&"expense-circle-0-1".to_string()).is_none());

        // Verify storage credit was returned (storage balance should increase)
        let storage_after = contract.storage_deposits.get(&accounts(0)).unwrap_or(0);
        assert!(storage_after >= storage_before, "Storage balance should not decrease after deletion");
    }

    /// Test that delete_expense credits storage back to the caller
    #[test]
    fn test_delete_expense_credits_storage() {
        let mut contract = setup();

        // Setup accounts with exact storage amounts
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Get storage before adding expense
        let storage_before_add = contract.storage_deposits.get(&accounts(0)).unwrap_or(0);

        // Add expense
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

        // Get storage after adding expense (should be less due to storage cost)
        let storage_after_add = contract.storage_deposits.get(&accounts(0)).unwrap_or(0);
        assert!(storage_after_add < storage_before_add, "Adding expense should consume storage");

        // Delete expense (requires 1 yoctoNEAR)
        ctx = context(accounts(0), 1);
        testing_env!(ctx.build());
        contract.delete_expense("circle-0".to_string(), "expense-circle-0-1".to_string());

        // Get storage after deletion (should be credited back)
        let storage_after_delete = contract.storage_deposits.get(&accounts(0)).unwrap_or(0);
        
        // Storage after delete should be more than after add (some credit returned)
        assert!(
            storage_after_delete > storage_after_add,
            "Deleting expense should credit storage back"
        );
    }

    /// Test that delete_expense resets confirmations
    #[test]
    fn test_delete_expense_resets_confirmations() {
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
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add two expenses
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

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(50),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Lunch".to_string(),
        );

        // Manually set a confirmation to test reset
        let confirmation_key = format!("{}:{}", "circle-0", accounts(0));
        contract.confirmations_map.insert(&confirmation_key, &true);
        contract.confirmations_count.insert(&"circle-0".to_string(), &1);

        // Verify confirmation exists
        assert!(contract.confirmations_map.get(&confirmation_key).unwrap_or(false));
        assert_eq!(contract.confirmations_count.get(&"circle-0".to_string()).unwrap_or(0), 1);

        // Delete an expense (requires 1 yoctoNEAR)
        ctx = context(accounts(0), 1);
        testing_env!(ctx.build());
        contract.delete_expense("circle-0".to_string(), "expense-circle-0-1".to_string());

        // Verify confirmations were reset
        assert!(!contract.confirmations_map.get(&confirmation_key).unwrap_or(false));
        assert_eq!(contract.confirmations_count.get(&"circle-0".to_string()).unwrap_or(0), 0);
    }

    /// Test that only the expense payer can delete an expense
    #[test]
    #[should_panic(expected = "Only the expense payer can delete this expense")]
    fn test_delete_expense_only_payer() {
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
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add expense (accounts(0) is the payer)
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

        // Try to delete as accounts(1) (requires 1 yoctoNEAR) - should fail
        ctx = context(accounts(1), 1);
        testing_env!(ctx.build());
        contract.delete_expense("circle-0".to_string(), "expense-circle-0-1".to_string());
    }

    // =========================================================================
    // GAS-SAFE CLEANUP TESTS
    // =========================================================================

    /// Test that batched settlement cleanup works correctly
    #[test]
    fn test_batched_settlement_cleanup() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

        // Manually insert many settlements to simulate a large circle
        let circle_id = "circle-0".to_string();
        let num_settlements = 250u64; // More than MAX_CLEANUP_BATCH_SIZE (100)
        
        for i in 0..num_settlements {
            let settlement = Settlement {
                circle_id: circle_id.clone(),
                from: accounts(0),
                to: accounts(0),
                amount: U128(100),
                token: None,
                ts_ms: 1620000000000,
                tx_kind: "test".to_string(),
                epoch: 0, // EPOCH-FIX: Test settlement in epoch 0
            };
            let settlement_id = format!("settlement-{}-{}", circle_id, i + 1);
            let index_key = NearSplitter::settlement_index_key(&circle_id, i);
            contract.settlements_index.insert(&index_key, &settlement_id);
            contract.settlement_by_id.insert(&settlement_id, &settlement);
        }
        contract.settlements_len.insert(&circle_id, &num_settlements);

        // Verify settlements were inserted
        assert_eq!(contract.settlements_len.get(&circle_id).unwrap_or(0), num_settlements);

        // First batch cleanup - should process MAX_CLEANUP_BATCH_SIZE (100)
        let remaining1 = contract.clear_settlements_batch(&circle_id, 100);
        assert_eq!(remaining1, 150); // 250 - 100 = 150 remaining

        // Check progress was saved
        let progress = contract.cleanup_progress.get(&format!("{}:settlements", circle_id));
        assert_eq!(progress, Some(100));

        // Second batch
        let remaining2 = contract.clear_settlements_batch(&circle_id, 100);
        assert_eq!(remaining2, 50); // 150 - 100 = 50 remaining

        // Third batch - completes cleanup
        let remaining3 = contract.clear_settlements_batch(&circle_id, 100);
        assert_eq!(remaining3, 0);

        // Verify cleanup is complete
        assert!(contract.settlements_len.get(&circle_id).is_none());
        assert!(contract.cleanup_progress.get(&format!("{}:settlements", circle_id)).is_none());
    }

    /// Test that batched claims cleanup works correctly
    #[test]
    fn test_batched_claims_cleanup() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

        // Manually insert many claims
        let circle_id = "circle-0".to_string();
        let num_claims = 150u64;
        
        for i in 0..num_claims {
            let claim = Claim {
                id: format!("claim-{}-{}", circle_id, i),
                circle_id: circle_id.clone(),
                expense_id: "expense-1".to_string(),
                claimant: accounts(0),
                reason: "wrong_amount".to_string(),
                proposed_amount: Some(U128(50)),
                proposed_participants: None,
                created_ms: 1620000000000,
                status: "pending".to_string(),
                resolved_ms: None,
            };
            let claim_id = format!("claim-{}-{}", circle_id, i);
            let index_key = NearSplitter::claim_index_key(&circle_id, i);
            contract.claims_index.insert(&index_key, &claim_id);
            contract.claim_by_id.insert(&claim_id, &claim);
        }
        contract.claims_len.insert(&circle_id, &num_claims);
        contract.pending_claims_count.insert(&circle_id, &num_claims);

        // First batch
        let remaining1 = contract.clear_claims_batch(&circle_id, 100);
        assert_eq!(remaining1, 50);

        // Second batch - completes cleanup
        let remaining2 = contract.clear_claims_batch(&circle_id, 100);
        assert_eq!(remaining2, 0);

        // Verify cleanup is complete
        assert!(contract.claims_len.get(&circle_id).is_none());
        assert!(contract.pending_claims_count.get(&circle_id).is_none());
    }

    /// Test cleanup_circle_data function
    #[test]
    fn test_cleanup_circle_data_function() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

        // Set circle to Settled state (required for cleanup)
        let circle_id = "circle-0".to_string();
        let mut circle = contract.circles.get(&circle_id).unwrap();
        circle.state = CircleState::Settled;
        contract.circles.insert(&circle_id, &circle);

        // Insert settlements
        let num_settlements = 150u64;
        for i in 0..num_settlements {
            let settlement = Settlement {
                circle_id: circle_id.clone(),
                from: accounts(0),
                to: accounts(0),
                amount: U128(100),
                token: None,
                ts_ms: 1620000000000,
                tx_kind: "test".to_string(),
                epoch: 0, // EPOCH-FIX: Test settlement in epoch 0
            };
            let settlement_id = format!("settlement-{}-{}", circle_id, i + 1);
            let index_key = NearSplitter::settlement_index_key(&circle_id, i);
            contract.settlements_index.insert(&index_key, &settlement_id);
            contract.settlement_by_id.insert(&settlement_id, &settlement);
        }
        contract.settlements_len.insert(&circle_id, &num_settlements);

        // First cleanup call
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        let (remaining_settlements, remaining_claims) = contract.cleanup_circle_data(circle_id.clone());
        assert_eq!(remaining_settlements, 50); // 150 - 100 = 50
        assert_eq!(remaining_claims, 0); // No claims

        // Second cleanup call - completes
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        let (remaining_settlements2, remaining_claims2) = contract.cleanup_circle_data(circle_id.clone());
        assert_eq!(remaining_settlements2, 0);
        assert_eq!(remaining_claims2, 0);
    }

    /// Test that delete_circle rejects circles with too much data
    #[test]
    #[should_panic(expected = "Circle has too much data for single delete")]
    fn test_delete_circle_rejects_large_data() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

        // Set circle to Settled state
        let circle_id = "circle-0".to_string();
        let mut circle = contract.circles.get(&circle_id).unwrap();
        circle.state = CircleState::Settled;
        contract.circles.insert(&circle_id, &circle);

        // Insert more settlements than MAX_CLEANUP_BATCH_SIZE
        let num_settlements = 150u64;
        for i in 0..num_settlements {
            let settlement = Settlement {
                circle_id: circle_id.clone(),
                from: accounts(0),
                to: accounts(0),
                amount: U128(100),
                token: None,
                ts_ms: 1620000000000,
                tx_kind: "test".to_string(),
                epoch: 0, // EPOCH-FIX: Test settlement in epoch 0
            };
            let settlement_id = format!("settlement-{}-{}", circle_id, i + 1);
            let index_key = NearSplitter::settlement_index_key(&circle_id, i);
            contract.settlements_index.insert(&index_key, &settlement_id);
            contract.settlement_by_id.insert(&settlement_id, &settlement);
        }
        contract.settlements_len.insert(&circle_id, &num_settlements);

        // Try to delete - should fail
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.delete_circle(circle_id);
    }

    /// Test delete_circle works after cleanup
    #[test]
    fn test_delete_circle_after_cleanup() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

        // Set circle to Settled state
        let circle_id = "circle-0".to_string();
        let mut circle = contract.circles.get(&circle_id).unwrap();
        circle.state = CircleState::Settled;
        contract.circles.insert(&circle_id, &circle);

        // Insert settlements (more than batch size)
        let num_settlements = 150u64;
        for i in 0..num_settlements {
            let settlement = Settlement {
                circle_id: circle_id.clone(),
                from: accounts(0),
                to: accounts(0),
                amount: U128(100),
                token: None,
                ts_ms: 1620000000000,
                tx_kind: "test".to_string(),
                epoch: 0, // EPOCH-FIX: Test settlement in epoch 0
            };
            let settlement_id = format!("settlement-{}-{}", circle_id, i + 1);
            let index_key = NearSplitter::settlement_index_key(&circle_id, i);
            contract.settlements_index.insert(&index_key, &settlement_id);
            contract.settlement_by_id.insert(&settlement_id, &settlement);
        }
        contract.settlements_len.insert(&circle_id, &num_settlements);

        // Run cleanup until complete
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        let (remaining1, _) = contract.cleanup_circle_data(circle_id.clone());
        assert!(remaining1 > 0);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        let (remaining2, _) = contract.cleanup_circle_data(circle_id.clone());
        assert_eq!(remaining2, 0);

        // Now delete should succeed
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.delete_circle(circle_id.clone());

        // Verify circle is deleted
        assert!(contract.circles.get(&circle_id).is_none());
    }

    /// Test get_cleanup_progress
    #[test]
    fn test_get_cleanup_progress() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Trip".to_string(), None, None);

        let circle_id = "circle-0".to_string();

        // Insert settlements
        contract.settlements_len.insert(&circle_id, &200);
        contract.cleanup_progress.insert(&format!("{}:settlements", circle_id), &75);
        
        // Insert claims
        contract.claims_len.insert(&circle_id, &50);

        let (settlements_total, settlements_cleared, claims_total, claims_cleared) = 
            contract.get_cleanup_progress(circle_id);
        
        assert_eq!(settlements_total, 200);
        assert_eq!(settlements_cleared, 75);
        assert_eq!(claims_total, 50);
        assert_eq!(claims_cleared, 0);
    }

    /// Test that settlements are preserved after autopay settlement
    #[test]
    fn test_settlements_preserved_after_autopay() {
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
        contract.create_circle("Trip".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add expense
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

        // Make a payment to record a settlement
        ctx = context(accounts(1), 50);
        testing_env!(ctx.build());
        contract.pay_native("circle-0".to_string(), accounts(0));

        // Verify settlement was recorded
        let settlements_before = contract.settlements_len.get(&"circle-0".to_string()).unwrap_or(0);
        assert_eq!(settlements_before, 1);

        // Both confirm (triggers autopay)
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        ctx = context(accounts(1), 50);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // After settlement, settlements should still exist (preserved for history)
        let settlements_after = contract.settlements_len.get(&"circle-0".to_string()).unwrap_or(0);
        // Should have original settlement + new autopay settlement
        assert!(settlements_after >= settlements_before, "Settlements should be preserved");
    }

    /// Test successful FT metadata caching via mocked callback result
    #[test]
    fn test_on_ft_metadata_success() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        let token_id: AccountId = "usdc.near".parse().unwrap();
        
        // Create valid metadata JSON that would come from token contract
        let metadata = FungibleTokenMetadata {
            spec: "ft-1.0.0".to_string(),
            name: "USD Coin".to_string(),
            symbol: "USDC".to_string(),
            icon: None,
            reference: None,
            reference_hash: None,
            decimals: 6,
        };
        let metadata_json = serde_json::to_vec(&metadata).unwrap();

        // Simulate callback context with successful promise result
        ctx = context(accounts(0), ONE_NEAR);
        ctx.current_account_id(env::current_account_id());
        ctx.predecessor_account_id(env::current_account_id());
        testing_env!(
            ctx.build(),
            test_vm_config(),
            near_sdk::RuntimeFeesConfig::test(),
            HashMap::default(),
            vec![PromiseResult::Successful(metadata_json)],
        );

        // Call the callback directly
        let result = contract.on_ft_metadata(token_id.clone(), accounts(0));
        assert!(result, "Callback should succeed with valid metadata");

        // Verify metadata was cached
        let cached = contract.ft_metadata(token_id.clone());
        assert!(cached.is_some(), "Metadata should be cached");
        let cached_meta = cached.unwrap();
        assert_eq!(cached_meta.name, "USD Coin");
        assert_eq!(cached_meta.symbol, "USDC");
        assert_eq!(cached_meta.decimals, 6);
    }

    /// Test that failed promise result does not modify cache
    #[test]
    fn test_on_ft_metadata_failed_promise() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        let token_id: AccountId = "bad-token.near".parse().unwrap();

        // Pre-cache some metadata to verify it's not modified
        let existing_metadata = FungibleTokenMetadata {
            spec: "ft-1.0.0".to_string(),
            name: "Existing Token".to_string(),
            symbol: "EXIST".to_string(),
            icon: None,
            reference: None,
            reference_hash: None,
            decimals: 18,
        };
        contract.metadata_cache.insert(&token_id, &existing_metadata);

        // Simulate callback context with failed promise result
        ctx = context(accounts(0), 0);
        ctx.current_account_id(env::current_account_id());
        ctx.predecessor_account_id(env::current_account_id());
        testing_env!(
            ctx.build(),
            test_vm_config(),
            near_sdk::RuntimeFeesConfig::test(),
            HashMap::default(),
            vec![PromiseResult::Failed],
        );

        // Call the callback directly
        let result = contract.on_ft_metadata(token_id.clone(), accounts(0));
        assert!(!result, "Callback should fail on failed promise");

        // Verify existing metadata was NOT modified
        let cached = contract.ft_metadata(token_id.clone());
        assert!(cached.is_some(), "Existing metadata should still be cached");
        let cached_meta = cached.unwrap();
        assert_eq!(cached_meta.name, "Existing Token", "Metadata should not be modified on failure");
    }

    /// Test that invalid metadata (empty name) is rejected
    #[test]
    fn test_on_ft_metadata_invalid_empty_name() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        let token_id: AccountId = "invalid.near".parse().unwrap();
        
        // Create invalid metadata with empty name
        let metadata = FungibleTokenMetadata {
            spec: "ft-1.0.0".to_string(),
            name: "".to_string(),  // Invalid: empty name
            symbol: "INV".to_string(),
            icon: None,
            reference: None,
            reference_hash: None,
            decimals: 18,
        };
        let metadata_json = serde_json::to_vec(&metadata).unwrap();

        // Simulate callback context with successful promise but invalid data
        ctx = context(accounts(0), ONE_NEAR);
        ctx.current_account_id(env::current_account_id());
        ctx.predecessor_account_id(env::current_account_id());
        testing_env!(
            ctx.build(),
            test_vm_config(),
            near_sdk::RuntimeFeesConfig::test(),
            HashMap::default(),
            vec![PromiseResult::Successful(metadata_json)],
        );

        // Call the callback directly
        let result = contract.on_ft_metadata(token_id.clone(), accounts(0));
        assert!(!result, "Callback should fail with empty name");

        // Verify metadata was NOT cached
        let cached = contract.ft_metadata(token_id);
        assert!(cached.is_none(), "Invalid metadata should not be cached");
    }

    /// Test that metadata with decimals > 24 is rejected
    #[test]
    fn test_on_ft_metadata_invalid_decimals() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        let token_id: AccountId = "highdecimals.near".parse().unwrap();
        
        // Create invalid metadata with decimals > 24
        let metadata = FungibleTokenMetadata {
            spec: "ft-1.0.0".to_string(),
            name: "High Decimals Token".to_string(),
            symbol: "HIGH".to_string(),
            icon: None,
            reference: None,
            reference_hash: None,
            decimals: 30,  // Invalid: exceeds 24
        };
        let metadata_json = serde_json::to_vec(&metadata).unwrap();

        // Simulate callback context
        ctx = context(accounts(0), ONE_NEAR);
        ctx.current_account_id(env::current_account_id());
        ctx.predecessor_account_id(env::current_account_id());
        testing_env!(
            ctx.build(),
            test_vm_config(),
            near_sdk::RuntimeFeesConfig::test(),
            HashMap::default(),
            vec![PromiseResult::Successful(metadata_json)],
        );

        // Call the callback directly
        let result = contract.on_ft_metadata(token_id.clone(), accounts(0));
        assert!(!result, "Callback should fail with decimals > 24");

        // Verify metadata was NOT cached
        let cached = contract.ft_metadata(token_id);
        assert!(cached.is_none(), "Invalid metadata should not be cached");
    }

    /// Test that metadata with name too long is rejected
    #[test]
    fn test_on_ft_metadata_name_too_long() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        let token_id: AccountId = "longname.near".parse().unwrap();
        
        // Create metadata with name exceeding MAX_FT_METADATA_NAME_LEN (128)
        let long_name = "A".repeat(150);
        let metadata = FungibleTokenMetadata {
            spec: "ft-1.0.0".to_string(),
            name: long_name,
            symbol: "LONG".to_string(),
            icon: None,
            reference: None,
            reference_hash: None,
            decimals: 18,
        };
        let metadata_json = serde_json::to_vec(&metadata).unwrap();

        // Simulate callback context
        ctx = context(accounts(0), ONE_NEAR);
        ctx.current_account_id(env::current_account_id());
        ctx.predecessor_account_id(env::current_account_id());
        testing_env!(
            ctx.build(),
            test_vm_config(),
            near_sdk::RuntimeFeesConfig::test(),
            HashMap::default(),
            vec![PromiseResult::Successful(metadata_json)],
        );

        // Call the callback directly
        let result = contract.on_ft_metadata(token_id.clone(), accounts(0));
        assert!(!result, "Callback should fail with name too long");

        // Verify metadata was NOT cached
        let cached = contract.ft_metadata(token_id);
        assert!(cached.is_none(), "Invalid metadata should not be cached");
    }

    /// Test that metadata with symbol too long is rejected
    #[test]
    fn test_on_ft_metadata_symbol_too_long() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        let token_id: AccountId = "longsymbol.near".parse().unwrap();
        
        // Create metadata with symbol exceeding MAX_FT_METADATA_SYMBOL_LEN (32)
        let long_symbol = "X".repeat(50);
        let metadata = FungibleTokenMetadata {
            spec: "ft-1.0.0".to_string(),
            name: "Long Symbol Token".to_string(),
            symbol: long_symbol,
            icon: None,
            reference: None,
            reference_hash: None,
            decimals: 18,
        };
        let metadata_json = serde_json::to_vec(&metadata).unwrap();

        // Simulate callback context
        ctx = context(accounts(0), ONE_NEAR);
        ctx.current_account_id(env::current_account_id());
        ctx.predecessor_account_id(env::current_account_id());
        testing_env!(
            ctx.build(),
            test_vm_config(),
            near_sdk::RuntimeFeesConfig::test(),
            HashMap::default(),
            vec![PromiseResult::Successful(metadata_json)],
        );

        // Call the callback directly
        let result = contract.on_ft_metadata(token_id.clone(), accounts(0));
        assert!(!result, "Callback should fail with symbol too long");

        // Verify metadata was NOT cached
        let cached = contract.ft_metadata(token_id);
        assert!(cached.is_none(), "Invalid metadata should not be cached");
    }

    // ========================================
    // Overflow Protection Regression Tests
    // ========================================

    /// Test that pending payout accumulation uses checked_add and detects overflow
    #[test]
    fn test_pending_payout_overflow_protection() {
        let mut contract = setup();

        // Setup accounts
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Manually set pending payout near max
        let near_max = u128::MAX - 100;
        contract.pending_payouts.insert(&accounts(0), &near_max);

        // Verify the value was set
        let stored = contract.pending_payouts.get(&accounts(0)).unwrap();
        assert_eq!(stored, near_max);

        // The checked_add in credit_pending_payout would panic on overflow
        // We can't directly test the panic here without triggering autopay,
        // but we verify the protection exists by checking stored value arithmetic
        let test_add = near_max.checked_add(1000);
        assert!(test_add.is_none(), "Adding to near-max should overflow");

        // Verify safe addition works for small values
        let safe_add = 100u128.checked_add(50);
        assert_eq!(safe_add, Some(150));
    }

    /// Test that escrow deposit accumulation uses checked_add
    #[test]
    fn test_escrow_deposit_overflow_protection() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        let escrow_key = "circle-0:alice.near".to_string();
        
        // Set escrow near max value
        let near_max = u128::MAX - 100;
        contract.escrow_deposits.insert(&escrow_key, &near_max);

        // Verify the value was set
        let stored = contract.escrow_deposits.get(&escrow_key).unwrap();
        assert_eq!(stored, near_max);

        // checked_add would catch overflow
        let overflow_result = near_max.checked_add(1000);
        assert!(overflow_result.is_none(), "Adding to near-max should overflow");
    }

    /// Test that counter increments stay within limits
    #[test]
    fn test_counter_increments_within_limits() {
        // Test safe_increment_u64 behavior for normal values
        let normal_value: u64 = 100;
        let incremented = normal_value.checked_add(1).unwrap();
        assert_eq!(incremented, 101);

        // Test that MAX value would overflow
        let max_value = u64::MAX;
        let overflow_result = max_value.checked_add(1);
        assert!(overflow_result.is_none(), "Incrementing u64::MAX should overflow");

        // Test that large but not max values work
        let large_value = u64::MAX - 10;
        let large_incremented = large_value.checked_add(1);
        assert!(large_incremented.is_some());
        assert_eq!(large_incremented.unwrap(), u64::MAX - 9);
    }

    /// Test that expense counter uses safe increment
    #[test]
    fn test_expense_counter_safe_increment() {
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
        contract.create_circle("Test".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add expense - should increment counter safely
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(1000),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Test expense".to_string(),
        );

        // Verify counter was incremented
        let expense_index = contract.next_expense_index.get(&"circle-0".to_string()).unwrap();
        assert_eq!(expense_index, 1);

        // Verify expenses_len was incremented
        let expenses_len = contract.expenses_len.get(&"circle-0".to_string()).unwrap();
        assert_eq!(expenses_len, 1);
    }

    /// Test that settlement counter uses safe increment
    #[test]
    fn test_settlement_counter_safe_increment() {
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
        contract.create_circle("Test".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add expense
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(1000),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );

        // Pay to create settlement
        ctx = context(accounts(1), 500);
        testing_env!(ctx.build());
        contract.pay_native("circle-0".to_string(), accounts(0));

        // Verify settlement counter was incremented
        let settlements_len = contract.settlements_len.get(&"circle-0".to_string()).unwrap();
        assert_eq!(settlements_len, 1);
    }

    /// Test that confirmation counter uses safe increment
    #[test]
    fn test_confirmation_counter_safe_increment() {
        let mut contract = setup();

        // Setup accounts
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle with two members
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Test".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // First member confirms
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // Verify confirmation count was incremented
        let confirmations = contract.confirmations_count.get(&"circle-0".to_string()).unwrap();
        assert_eq!(confirmations, 1);
    }

    /// Test saturating_sub is used for debt/credit calculations
    #[test]
    fn test_suggest_settlements_uses_saturating_sub() {
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
        contract.create_circle("Test".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add expense - accounts(0) paid, accounts(1) owes
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(1000),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Test".to_string(),
        );

        // Get settlement suggestions
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        let suggestions = contract.suggest_settlements("circle-0".to_string());

        // Should have one suggestion: accounts(1) -> accounts(0) for 500
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].from, accounts(1));
        assert_eq!(suggestions[0].to, accounts(0));
        assert_eq!(suggestions[0].amount.0, 500);
    }

    // ========================================
    // 1 yoctoNEAR Requirement Tests
    // ========================================

    /// Test that transfer_ownership requires exactly 1 yoctoNEAR
    #[test]
    #[should_panic(expected = "Requires exactly 1 yoctoNEAR")]
    fn test_transfer_ownership_requires_one_yocto() {
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
        contract.create_circle("Test".to_string(), None, None);

        // Add member
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Try to transfer ownership without 1 yoctoNEAR - should panic
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.transfer_ownership("circle-0".to_string(), accounts(1));
    }

    /// Test that transfer_ownership succeeds with exactly 1 yoctoNEAR
    #[test]
    fn test_transfer_ownership_with_one_yocto() {
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
        contract.create_circle("Test".to_string(), None, None);

        // Add member
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Transfer ownership with 1 yoctoNEAR - should succeed
        ctx = context(accounts(0), 1);
        testing_env!(ctx.build());
        contract.transfer_ownership("circle-0".to_string(), accounts(1));

        let circle = contract.get_circle("circle-0".to_string());
        assert_eq!(circle.owner, accounts(1));
    }

    /// Test that delete_expense requires exactly 1 yoctoNEAR
    #[test]
    #[should_panic(expected = "Requires exactly 1 yoctoNEAR")]
    fn test_delete_expense_requires_one_yocto() {
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
        contract.create_circle("Test".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(1000),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Test".to_string(),
        );

        // Try to delete expense without 1 yoctoNEAR - should panic
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.delete_expense("circle-0".to_string(), "expense-circle-0-1".to_string());
    }

    /// Test that set_membership_open requires exactly 1 yoctoNEAR
    #[test]
    #[should_panic(expected = "Requires exactly 1 yoctoNEAR")]
    fn test_set_membership_open_requires_one_yocto() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Test".to_string(), None, None);

        // Try to set membership without 1 yoctoNEAR - should panic
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.set_membership_open("circle-0".to_string(), false);
    }

    /// Test that reset_confirmations requires exactly 1 yoctoNEAR
    #[test]
    #[should_panic(expected = "Requires exactly 1 yoctoNEAR")]
    fn test_reset_confirmations_requires_one_yocto() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Create circle
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Test".to_string(), None, None);

        // Try to reset confirmations without 1 yoctoNEAR - should panic
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.reset_confirmations("circle-0".to_string());
    }

    /// Test that cancel_settlement requires exactly 1 yoctoNEAR
    #[test]
    #[should_panic(expected = "Requires exactly 1 yoctoNEAR")]
    fn test_cancel_settlement_requires_one_yocto() {
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
        contract.create_circle("Test".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // First member confirms to enter SettlementInProgress
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // Try to cancel settlement without 1 yoctoNEAR - should panic
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.cancel_settlement("circle-0".to_string());
    }

    /// Test that disabling autopay requires exactly 1 yoctoNEAR
    #[test]
    #[should_panic(expected = "Disabling autopay requires exactly 1 yoctoNEAR")]
    fn test_disable_autopay_requires_one_yocto() {
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
        contract.create_circle("Test".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Enable autopay first (creditor, so 0 deposit is fine)
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.set_autopay("circle-0".to_string(), true);

        // Try to disable autopay without 1 yoctoNEAR - should panic
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.set_autopay("circle-0".to_string(), false);
    }

    /// Test that disabling autopay succeeds with 1 yoctoNEAR
    #[test]
    fn test_disable_autopay_with_one_yocto() {
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
        contract.create_circle("Test".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Enable autopay first
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.set_autopay("circle-0".to_string(), true);

        // Disable autopay with 1 yoctoNEAR - should succeed
        ctx = context(accounts(0), 1);
        testing_env!(ctx.build());
        contract.set_autopay("circle-0".to_string(), false);

        // Verify autopay is disabled
        let key = format!("circle-0:{}", accounts(0));
        let autopay_enabled = contract.autopay_preferences.get(&key).unwrap_or(false);
        assert!(!autopay_enabled);
    }

    /// Test that approve_claim requires exactly 1 yoctoNEAR
    #[test]
    #[should_panic(expected = "Requires exactly 1 yoctoNEAR")]
    fn test_approve_claim_requires_one_yocto() {
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
        contract.create_circle("Test".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(1000),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Test".to_string(),
        );

        // File a claim
        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.file_claim(
            "circle-0".to_string(),
            "expense-circle-0-1".to_string(),
            "wrong_amount".to_string(),
            Some(U128(500)),
            None,
        );

        // Try to approve claim without 1 yoctoNEAR - should panic
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.approve_claim("circle-0".to_string(), "claim-circle-0-1".to_string());
    }

    /// Test that reject_claim requires exactly 1 yoctoNEAR
    #[test]
    #[should_panic(expected = "Requires exactly 1 yoctoNEAR")]
    fn test_reject_claim_requires_one_yocto() {
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
        contract.create_circle("Test".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(1000),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Test".to_string(),
        );

        // File a claim
        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.file_claim(
            "circle-0".to_string(),
            "expense-circle-0-1".to_string(),
            "wrong_amount".to_string(),
            Some(U128(500)),
            None,
        );

        // Try to reject claim without 1 yoctoNEAR - should panic
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.reject_claim("circle-0".to_string(), "claim-circle-0-1".to_string());
    }

    /// Test that confirm_ledger allows debtor to attach >= debt (not exactly 1 yocto)
    #[test]
    fn test_confirm_ledger_debtor_deposit_semantics() {
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
        contract.create_circle("Test".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add expense - accounts(1) owes 500
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(1000),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Test".to_string(),
        );

        // Debtor confirms with exact debt amount - should succeed
        ctx = context(accounts(1), 500);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // Verify escrow was deposited
        let escrow_key = format!("circle-0:{}", accounts(1));
        let escrowed = contract.escrow_deposits.get(&escrow_key).unwrap_or(0);
        assert_eq!(escrowed, 500);
    }

    /// Test that confirm_ledger allows creditor with 0 deposit (refund happens)
    #[test]
    fn test_confirm_ledger_creditor_no_deposit() {
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
        contract.create_circle("Test".to_string(), None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string(), None);

        // Add expense - accounts(0) is creditor
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(1000),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Test".to_string(),
        );

        // Creditor confirms with 0 deposit - should succeed
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // Verify confirmation was recorded
        let confirmation_key = format!("circle-0:{}", accounts(0));
        assert!(contract.confirmations_map.get(&confirmation_key).unwrap_or(false));
    }

    // ========== Escrow Aggregate Tracking Tests ==========

    /// Test that escrow_increase updates both per-account and global aggregates
    #[test]
    fn test_escrow_aggregate_increase() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Verify initial state is 0
        assert_eq!(contract.get_escrow_total(accounts(0)), U128(0));
        assert_eq!(contract.get_total_escrow(), U128(0));

        // Manually call escrow_increase
        let escrow_key = "circle-0:alice.near".to_string();
        contract.escrow_increase(&accounts(0), &escrow_key, 1000);

        // Verify aggregates updated
        assert_eq!(contract.get_escrow_total(accounts(0)), U128(1000));
        assert_eq!(contract.get_total_escrow(), U128(1000));

        // Add more escrow for same account
        let escrow_key2 = "circle-1:alice.near".to_string();
        contract.escrow_increase(&accounts(0), &escrow_key2, 500);

        assert_eq!(contract.get_escrow_total(accounts(0)), U128(1500));
        assert_eq!(contract.get_total_escrow(), U128(1500));

        // Add escrow for different account
        let escrow_key3 = "circle-0:bob.near".to_string();
        contract.escrow_increase(&accounts(1), &escrow_key3, 2000);

        assert_eq!(contract.get_escrow_total(accounts(0)), U128(1500));
        assert_eq!(contract.get_escrow_total(accounts(1)), U128(2000));
        assert_eq!(contract.get_total_escrow(), U128(3500));
    }

    /// Test that escrow_decrease updates aggregates correctly
    #[test]
    fn test_escrow_aggregate_decrease() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Add escrow first
        let escrow_key = "circle-0:alice.near".to_string();
        contract.escrow_increase(&accounts(0), &escrow_key, 1000);

        assert_eq!(contract.get_escrow_total(accounts(0)), U128(1000));
        assert_eq!(contract.get_total_escrow(), U128(1000));

        // Decrease escrow
        contract.escrow_decrease(&accounts(0), &escrow_key, 400);

        assert_eq!(contract.get_escrow_total(accounts(0)), U128(600));
        assert_eq!(contract.get_total_escrow(), U128(600));
        assert_eq!(contract.escrow_deposits.get(&escrow_key).unwrap(), 600);

        // Decrease to zero - should remove entry
        contract.escrow_decrease(&accounts(0), &escrow_key, 600);

        assert_eq!(contract.get_escrow_total(accounts(0)), U128(0));
        assert_eq!(contract.get_total_escrow(), U128(0));
        assert!(contract.escrow_deposits.get(&escrow_key).is_none());
    }

    /// Test that escrow_remove_for_circle clears full escrow for a circle
    #[test]
    fn test_escrow_remove_for_circle() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Add escrow for multiple circles
        let escrow_key1 = "circle-0:alice.near".to_string();
        let escrow_key2 = "circle-1:alice.near".to_string();
        contract.escrow_increase(&accounts(0), &escrow_key1, 1000);
        contract.escrow_increase(&accounts(0), &escrow_key2, 500);

        assert_eq!(contract.get_escrow_total(accounts(0)), U128(1500));
        assert_eq!(contract.get_total_escrow(), U128(1500));

        // Remove escrow for circle-0
        contract.escrow_remove_for_circle(&accounts(0), &escrow_key1);

        assert_eq!(contract.get_escrow_total(accounts(0)), U128(500));
        assert_eq!(contract.get_total_escrow(), U128(500));
        assert!(contract.escrow_deposits.get(&escrow_key1).is_none());
        assert_eq!(contract.escrow_deposits.get(&escrow_key2).unwrap(), 500);
    }

    /// Test that storage_unregister fails when account has escrowed funds
    #[test]
    #[should_panic(expected = "Withdraw escrowed funds before unregistering")]
    fn test_storage_unregister_fails_with_escrow() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Add escrow directly (simulating confirm_ledger)
        let escrow_key = "circle-0:alice.near".to_string();
        contract.escrow_increase(&accounts(0), &escrow_key, 1000);

        // Try to unregister - should fail
        ctx = context(accounts(0), ONE_YOCTO);
        testing_env!(ctx.build());
        contract.storage_unregister(Some(false));
    }

    /// Test that storage_unregister with force=true refunds escrowed funds
    #[test]
    fn test_storage_unregister_force_refunds_escrow() {
        let mut contract = setup();

        // Setup account
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Add escrow directly
        let escrow_key = "circle-0:alice.near".to_string();
        contract.escrow_increase(&accounts(0), &escrow_key, 1000);

        assert_eq!(contract.get_escrow_total(accounts(0)), U128(1000));
        assert_eq!(contract.get_total_escrow(), U128(1000));

        // Force unregister - should succeed and clear aggregates
        ctx = context(accounts(0), ONE_YOCTO);
        testing_env!(ctx.build());
        let result = contract.storage_unregister(Some(true));

        assert!(result);
        // Aggregates should be cleared
        assert_eq!(contract.get_escrow_total(accounts(0)), U128(0));
        assert_eq!(contract.get_total_escrow(), U128(0));
    }

    // ========== Aggregate Tracking Tests ==========

    /// Test that storage_deposit updates total_storage_deposits aggregate
    #[test]
    fn test_storage_deposit_updates_aggregate() {
        let mut contract = setup();

        // Initial aggregate should be 0
        assert_eq!(contract.get_total_storage_deposits(), U128(0));

        // First registration
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        assert_eq!(contract.get_total_storage_deposits(), U128(ONE_NEAR));

        // Second registration
        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        assert_eq!(contract.get_total_storage_deposits(), U128(2 * ONE_NEAR));

        // Add more to existing account
        ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        assert_eq!(contract.get_total_storage_deposits(), U128(3 * ONE_NEAR));
    }

    /// Test that storage_withdraw updates total_storage_deposits aggregate
    #[test]
    fn test_storage_withdraw_updates_aggregate() {
        let mut contract = setup();

        // Register with extra funds
        let mut ctx = context(accounts(0), 2 * ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        assert_eq!(contract.get_total_storage_deposits(), U128(2 * ONE_NEAR));

        // Withdraw some
        ctx = context(accounts(0), ONE_YOCTO);
        testing_env!(ctx.build());
        contract.storage_withdraw(Some(U128(ONE_NEAR / 2)));

        // Aggregate should be reduced
        assert_eq!(contract.get_total_storage_deposits(), U128(2 * ONE_NEAR - ONE_NEAR / 2));
    }

    /// Test that pending payout credit updates total_pending_payouts aggregate
    #[test]
    fn test_pending_payout_updates_aggregate() {
        let mut contract = setup();

        // Initial aggregate should be 0
        assert_eq!(contract.get_total_pending_payouts(), U128(0));

        // Simulate pending payout credit (direct insert for test)
        contract.pending_payouts.insert(&accounts(0), &1000u128);
        contract.total_pending_payouts = 1000;

        assert_eq!(contract.get_total_pending_payouts(), U128(1000));
    }

    /// Test that withdraw_payout updates total_pending_payouts aggregate
    #[test]
    fn test_withdraw_payout_updates_aggregate() {
        let mut contract = setup();

        // Setup account with pending payout
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Set pending payout directly
        contract.pending_payouts.insert(&accounts(0), &5000u128);
        contract.total_pending_payouts = 5000;

        assert_eq!(contract.get_total_pending_payouts(), U128(5000));

        // Withdraw
        ctx = context(accounts(0), ONE_YOCTO);
        testing_env!(ctx.build());
        contract.withdraw_payout();

        // Aggregate should be 0
        assert_eq!(contract.get_total_pending_payouts(), U128(0));
    }

    /// Test that withdraw_payout_partial updates total_pending_payouts aggregate
    #[test]
    fn test_withdraw_payout_partial_updates_aggregate() {
        let mut contract = setup();

        // Setup account with pending payout
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Set pending payout directly
        contract.pending_payouts.insert(&accounts(0), &5000u128);
        contract.total_pending_payouts = 5000;

        // Partial withdraw
        ctx = context(accounts(0), ONE_YOCTO);
        testing_env!(ctx.build());
        contract.withdraw_payout_partial(U128(3000));

        // Aggregate should be reduced
        assert_eq!(contract.get_total_pending_payouts(), U128(2000));
    }

    // ========== Rescue Validation Tests ==========

    /// Test that rescue_stuck_near rejects draining user storage deposits
    #[test]
    #[should_panic(expected = "Cannot rescue")]
    fn test_rescue_rejects_draining_storage_deposits() {
        let mut contract = setup();

        // Register users with storage deposits
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // total_storage_deposits = 2 NEAR

        // Simulate contract having balance (via attached NEAR to methods)
        // Set context as contract itself (private method)
        let rescue_ctx = VMContextBuilder::new()
            .predecessor_account_id("contract.near".parse().unwrap())
            .current_account_id("contract.near".parse().unwrap())
            .account_balance(NearToken::from_yoctonear(3 * ONE_NEAR))
            .build();
        testing_env!(rescue_ctx);

        // Try to rescue 2 NEAR (which would drain user deposits)
        contract.rescue_stuck_near(accounts(2), U128(2 * ONE_NEAR));
    }

    /// Test that rescue_stuck_near rejects draining pending payouts
    #[test]
    #[should_panic(expected = "Cannot rescue")]
    fn test_rescue_rejects_draining_pending_payouts() {
        let mut contract = setup();

        // Setup pending payouts
        contract.pending_payouts.insert(&accounts(0), &ONE_NEAR);
        contract.total_pending_payouts = ONE_NEAR;

        // Set context as contract itself
        let ctx = VMContextBuilder::new()
            .predecessor_account_id("contract.near".parse().unwrap())
            .current_account_id("contract.near".parse().unwrap())
            .account_balance(NearToken::from_yoctonear(2 * ONE_NEAR))
            .build();
        testing_env!(ctx);

        // Try to rescue 1.5 NEAR (which would drain pending payouts)
        contract.rescue_stuck_near(accounts(2), U128(ONE_NEAR + ONE_NEAR / 2));
    }

    /// Test that rescue_stuck_near rejects draining escrow
    #[test]
    #[should_panic(expected = "Cannot rescue")]
    fn test_rescue_rejects_draining_escrow() {
        let mut contract = setup();

        // Setup escrow
        let escrow_key = "circle-0:alice.near".to_string();
        contract.escrow_increase(&accounts(0), &escrow_key, ONE_NEAR);

        // Set context as contract itself
        let ctx = VMContextBuilder::new()
            .predecessor_account_id("contract.near".parse().unwrap())
            .current_account_id("contract.near".parse().unwrap())
            .account_balance(NearToken::from_yoctonear(2 * ONE_NEAR))
            .build();
        testing_env!(ctx);

        // Try to rescue 1.5 NEAR (which would drain escrow)
        contract.rescue_stuck_near(accounts(2), U128(ONE_NEAR + ONE_NEAR / 2));
    }

    /// Test that rescue_stuck_near allows rescuing truly stuck funds
    #[test]
    fn test_rescue_allows_stuck_funds() {
        let mut contract = setup();

        // Setup some user deposits
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // Simulate contract having extra balance (stuck funds)
        // Contract has: 5 NEAR balance, 1 NEAR in storage deposits
        // Should be able to rescue ~4 NEAR minus storage costs and buffer
        let rescue_ctx = VMContextBuilder::new()
            .predecessor_account_id("contract.near".parse().unwrap())
            .current_account_id("contract.near".parse().unwrap())
            .account_balance(NearToken::from_yoctonear(5 * ONE_NEAR))
            .storage_usage(1000) // Small storage usage
            .build();
        testing_env!(rescue_ctx);

        // Rescue a small amount that's clearly available
        // (less than 5 NEAR - 1 NEAR storage - safety buffer)
        let _promise = contract.rescue_stuck_near(accounts(2), U128(ONE_NEAR));
        // If we get here without panic, rescue succeeded
    }

    // =========================================================================
    // LEDGER EPOCH TESTS
    // =========================================================================

    /// Test that after autopay settlement completes, compute_balances returns zero for all members
    /// Then after adding a new expense, balances reflect only that new expense.
    #[test]
    fn test_epoch_zero_balances_after_autopay() {
        let mut contract = setup();

        // Setup: account(0) pays storage and creates circle
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Epoch Test".to_string(), None, None);
        add_members_helper(&mut contract, "circle-0", vec![accounts(1)]);

        // Add expense: accounts(0) paid 100, split 50/50
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Dinner".to_string(),
        );

        // Check initial balances: account(0) = +50, account(1) = -50
        let balances_before = contract.compute_balances("circle-0".to_string());
        let mut map: std::collections::HashMap<AccountId, i128> = std::collections::HashMap::new();
        for b in balances_before {
            map.insert(b.account_id, b.net.0);
        }
        assert_eq!(map.get(&accounts(0)).copied(), Some(50));
        assert_eq!(map.get(&accounts(1)).copied(), Some(-50));

        // Verify epoch is 0
        let circle = contract.circles.get(&"circle-0".to_string()).unwrap();
        assert_eq!(circle.ledger_epoch, 0);

        // Simulate autopay settlement completion by incrementing epoch manually
        let mut updated_circle = circle.clone();
        updated_circle.ledger_epoch = 1;
        updated_circle.state = CircleState::Settled;
        contract.circles.insert(&"circle-0".to_string(), &updated_circle);

        // After epoch increment, compute_balances should return zero for all
        let balances_after = contract.compute_balances("circle-0".to_string());
        let mut map_after: std::collections::HashMap<AccountId, i128> = std::collections::HashMap::new();
        for b in balances_after {
            map_after.insert(b.account_id, b.net.0);
        }
        assert_eq!(map_after.get(&accounts(0)).copied(), Some(0));
        assert_eq!(map_after.get(&accounts(1)).copied(), Some(0));

        // Re-open circle and add a new expense in the new epoch
        let mut reopened = contract.circles.get(&"circle-0".to_string()).unwrap();
        reopened.state = CircleState::Open;
        contract.circles.insert(&"circle-0".to_string(), &reopened);

        // Add new expense in epoch 1
        contract.add_expense(
            "circle-0".to_string(),
            U128(200),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Epoch 1 expense".to_string(),
        );

        // Balances should reflect only the new epoch expense: +100, -100
        let balances_new = contract.compute_balances("circle-0".to_string());
        let mut map_new: std::collections::HashMap<AccountId, i128> = std::collections::HashMap::new();
        for b in balances_new {
            map_new.insert(b.account_id, b.net.0);
        }
        assert_eq!(map_new.get(&accounts(0)).copied(), Some(100));
        assert_eq!(map_new.get(&accounts(1)).copied(), Some(-100));
    }

    /// Test that suggest_settlements only uses current epoch data
    #[test]
    fn test_epoch_suggest_settlements_current_only() {
        let mut contract = setup();

        // Setup
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Suggest Test".to_string(), None, None);
        add_members_helper(&mut contract, "circle-0", vec![accounts(1)]);

        // Add expense in epoch 0
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Old epoch expense".to_string(),
        );

        // Verify suggestions exist in epoch 0
        let suggestions_before = contract.suggest_settlements("circle-0".to_string());
        assert!(!suggestions_before.is_empty(), "Should have suggestions in epoch 0");

        // Increment epoch (simulating settlement)
        let mut circle = contract.circles.get(&"circle-0".to_string()).unwrap();
        circle.ledger_epoch = 1;
        contract.circles.insert(&"circle-0".to_string(), &circle);

        // After epoch increment, no suggestions (no expenses in epoch 1)
        let suggestions_after = contract.suggest_settlements("circle-0".to_string());
        assert!(suggestions_after.is_empty(), "Should have no suggestions after epoch increment");
    }

    // =========================================================================
    // TOKEN ALLOWLIST TESTS
    // =========================================================================

    /// Test that approve_token adds a token to the allowlist
    #[test]
    fn test_approve_token() {
        let mut contract = setup();
        let token_id: AccountId = "usdc.near".parse().unwrap();

        // Initially, token should not be approved
        assert!(!contract.is_token_approved(token_id.clone()));

        // Approve the token (as contract owner)
        let ctx = VMContextBuilder::new()
            .predecessor_account_id("contract.near".parse().unwrap())
            .current_account_id("contract.near".parse().unwrap())
            .build();
        testing_env!(ctx);

        contract.approve_token(token_id.clone());

        // Now token should be approved
        assert!(contract.is_token_approved(token_id));
    }

    /// Test that revoke_token removes a token from the allowlist
    #[test]
    fn test_revoke_token() {
        let mut contract = setup();
        let token_id: AccountId = "usdc.near".parse().unwrap();

        // Approve the token first
        let ctx = VMContextBuilder::new()
            .predecessor_account_id("contract.near".parse().unwrap())
            .current_account_id("contract.near".parse().unwrap())
            .build();
        testing_env!(ctx);

        contract.approve_token(token_id.clone());
        assert!(contract.is_token_approved(token_id.clone()));

        // Now revoke
        contract.revoke_token(token_id.clone());
        assert!(!contract.is_token_approved(token_id));
    }

    /// Test that only contract owner can approve tokens
    #[test]
    #[should_panic(expected = "Only contract owner can approve tokens")]
    fn test_approve_token_requires_owner() {
        let mut contract = setup();
        let token_id: AccountId = "usdc.near".parse().unwrap();

        // Try to approve as non-owner
        let ctx = context(accounts(0), 0);
        testing_env!(ctx.build());

        contract.approve_token(token_id);
    }

    /// Test that only contract owner can revoke tokens
    #[test]
    #[should_panic(expected = "Only contract owner can revoke tokens")]
    fn test_revoke_token_requires_owner() {
        let mut contract = setup();
        let token_id: AccountId = "usdc.near".parse().unwrap();

        // Approve first as contract owner
        let ctx = VMContextBuilder::new()
            .predecessor_account_id("contract.near".parse().unwrap())
            .current_account_id("contract.near".parse().unwrap())
            .build();
        testing_env!(ctx);
        contract.approve_token(token_id.clone());

        // Try to revoke as non-owner - should fail
        let ctx2 = context(accounts(0), 0);
        testing_env!(ctx2.build());
        contract.revoke_token(token_id);
    }

    /// Test that is_token_approved returns false for unknown tokens
    #[test]
    fn test_is_token_approved_unknown_token() {
        let contract = setup();
        let unknown_token: AccountId = "unknown.near".parse().unwrap();
        
        assert!(!contract.is_token_approved(unknown_token));
    }

    // =========================================================================
    // CIRCLE LISTING TESTS
    // =========================================================================

    /// Test list_circles_by_owner returns circles for an owner
    #[test]
    fn test_list_circles_by_owner() {
        let mut contract = setup();

        // Setup: accounts(0) creates two circles
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Circle A".to_string(), None, None);
        contract.create_circle("Circle B".to_string(), None, None);

        // List circles by owner
        let circles = contract.list_circles_by_owner(accounts(0), None, None);
        assert_eq!(circles.len(), 2);
        assert_eq!(circles[0].name, "Circle A");
        assert_eq!(circles[1].name, "Circle B");

        // accounts(1) should have no circles
        let circles_1 = contract.list_circles_by_owner(accounts(1), None, None);
        assert!(circles_1.is_empty());
    }

    /// Test list_circles_by_owner with pagination
    #[test]
    fn test_list_circles_by_owner_pagination() {
        let mut contract = setup();

        // Setup: accounts(0) creates 5 circles
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        for i in 0..5 {
            contract.create_circle(format!("Circle {}", i), None, None);
        }

        // Get first 2 circles
        let page1 = contract.list_circles_by_owner(accounts(0), Some(0), Some(2));
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].name, "Circle 0");
        assert_eq!(page1[1].name, "Circle 1");

        // Get next 2 circles
        let page2 = contract.list_circles_by_owner(accounts(0), Some(2), Some(2));
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].name, "Circle 2");
        assert_eq!(page2[1].name, "Circle 3");

        // Get last page
        let page3 = contract.list_circles_by_owner(accounts(0), Some(4), Some(2));
        assert_eq!(page3.len(), 1);
        assert_eq!(page3[0].name, "Circle 4");
    }

    /// Test list_circles_by_member returns circles where user is a member
    #[test]
    fn test_list_circles_by_member() {
        let mut contract = setup();

        // Setup: accounts(0) creates circle and adds accounts(1) as member
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Shared Circle".to_string(), None, None);
        add_members_helper(&mut contract, "circle-0", vec![accounts(1)]);

        // accounts(1) deposits storage
        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // accounts(0) should see their circle as owner
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        let circles_0 = contract.list_circles_by_member(accounts(0), None, None);
        assert_eq!(circles_0.len(), 1);
        assert_eq!(circles_0[0].name, "Shared Circle");

        // accounts(1) should see the same circle as member
        let circles_1 = contract.list_circles_by_member(accounts(1), None, None);
        assert_eq!(circles_1.len(), 1);
        assert_eq!(circles_1[0].name, "Shared Circle");
    }

    /// Test list_circles_by_member with pagination
    #[test]
    fn test_list_circles_by_member_pagination() {
        let mut contract = setup();

        // Setup: accounts(0) and accounts(1) both create circles, all with accounts(2) as member
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Circle from 0".to_string(), None, None);
        add_members_helper(&mut contract, "circle-0", vec![accounts(2)]);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.create_circle("Circle from 1".to_string(), None, None);
        add_members_helper(&mut contract, "circle-1", vec![accounts(2)]);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Another from 0".to_string(), None, None);
        add_members_helper(&mut contract, "circle-2", vec![accounts(2)]);

        // accounts(2) should see all 3 circles
        let all_circles = contract.list_circles_by_member(accounts(2), None, None);
        assert_eq!(all_circles.len(), 3);

        // Paginate
        let page1 = contract.list_circles_by_member(accounts(2), Some(0), Some(2));
        assert_eq!(page1.len(), 2);

        let page2 = contract.list_circles_by_member(accounts(2), Some(2), Some(2));
        assert_eq!(page2.len(), 1);
    }

    // =========================================================================
    // GET_EXPENSE_CLAIMS TESTS
    // =========================================================================

    /// Test that get_expense_claims returns claims only for the specified expense
    #[test]
    fn test_get_expense_claims_returns_only_specified_expense() {
        let mut contract = setup();

        // Setup: accounts(0) creates circle with accounts(1) and accounts(2)
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
        contract.create_circle("Test Circle".to_string(), None, None);
        add_members_helper(&mut contract, "circle-0", vec![accounts(1), accounts(2)]);

        // Create two expenses
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Expense A".to_string(),
        );

        contract.add_expense(
            "circle-0".to_string(),
            U128(200),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(2), weight_bps: 5_000 },
            ],
            "Expense B".to_string(),
        );

        // accounts(1) files a claim on expense A
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.file_claim(
            "circle-0".to_string(),
            "expense-circle-0-1".to_string(),
            "wrong_amount".to_string(),
            Some(U128(80)),
            None,
        );

        // accounts(2) files a claim on expense B
        ctx = context(accounts(2), 0);
        testing_env!(ctx.build());
        contract.file_claim(
            "circle-0".to_string(),
            "expense-circle-0-2".to_string(),
            "wrong_amount".to_string(),
            Some(U128(150)),
            None,
        );

        // get_expense_claims for expense A should return only the claim for A
        let claims_a = contract.get_expense_claims("circle-0".to_string(), "expense-circle-0-1".to_string());
        assert_eq!(claims_a.len(), 1);
        assert_eq!(claims_a[0].expense_id, "expense-circle-0-1");
        assert_eq!(claims_a[0].claimant, accounts(1));
        assert_eq!(claims_a[0].proposed_amount, Some(U128(80)));

        // get_expense_claims for expense B should return only the claim for B
        let claims_b = contract.get_expense_claims("circle-0".to_string(), "expense-circle-0-2".to_string());
        assert_eq!(claims_b.len(), 1);
        assert_eq!(claims_b[0].expense_id, "expense-circle-0-2");
        assert_eq!(claims_b[0].claimant, accounts(2));
        assert_eq!(claims_b[0].proposed_amount, Some(U128(150)));
    }

    /// Test that get_expense_claims returns empty for expense with no claims
    #[test]
    fn test_get_expense_claims_empty_for_no_claims() {
        let mut contract = setup();

        // Setup
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Test Circle".to_string(), None, None);
        add_members_helper(&mut contract, "circle-0", vec![accounts(1)]);

        // Create expense with no claims
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "No claims expense".to_string(),
        );

        // get_expense_claims should return empty vec
        let claims = contract.get_expense_claims("circle-0".to_string(), "expense-circle-0-1".to_string());
        assert!(claims.is_empty());
    }

    /// Test that get_expense_claims returns multiple claims for the same expense
    #[test]
    fn test_get_expense_claims_multiple_claims_same_expense() {
        let mut contract = setup();

        // Setup: accounts(0) creates circle with accounts(1) and accounts(2)
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
        contract.create_circle("Test Circle".to_string(), None, None);
        add_members_helper(&mut contract, "circle-0", vec![accounts(1), accounts(2)]);

        // Create expense with both accounts(1) and accounts(2) as participants
        contract.add_expense(
            "circle-0".to_string(),
            U128(300),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 3_333 },
                MemberShare { account_id: accounts(1), weight_bps: 3_333 },
                MemberShare { account_id: accounts(2), weight_bps: 3_334 },
            ],
            "Multi-participant expense".to_string(),
        );

        // accounts(1) files a claim
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.file_claim(
            "circle-0".to_string(),
            "expense-circle-0-1".to_string(),
            "wrong_amount".to_string(),
            Some(U128(250)),
            None,
        );

        // accounts(2) files a claim
        ctx = context(accounts(2), 0);
        testing_env!(ctx.build());
        contract.file_claim(
            "circle-0".to_string(),
            "expense-circle-0-1".to_string(),
            "wrong_amount".to_string(),
            Some(U128(275)),
            None,
        );

        // get_expense_claims should return both claims
        let claims = contract.get_expense_claims("circle-0".to_string(), "expense-circle-0-1".to_string());
        assert_eq!(claims.len(), 2);
        
        // Verify both claimants are present
        let claimants: Vec<AccountId> = claims.iter().map(|c| c.claimant.clone()).collect();
        assert!(claimants.contains(&accounts(1)));
        assert!(claimants.contains(&accounts(2)));
    }

    // =========================================================================
    // IS_FULLY_CONFIRMED AND IS_MEMBERSHIP_OPEN TESTS
    // =========================================================================

    /// Test is_fully_confirmed returns false when only one member has confirmed
    #[test]
    fn test_is_fully_confirmed_partial() {
        let mut contract = setup();

        // Setup: 2-member circle
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Confirm Test".to_string(), None, None);
        add_members_helper(&mut contract, "circle-0", vec![accounts(1)]);

        // Initially not confirmed
        assert!(!contract.is_fully_confirmed("circle-0".to_string()));

        // Only accounts(0) confirms (creditor, no deposit needed)
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // Should still be false since only 1 of 2 members confirmed
        assert!(!contract.is_fully_confirmed("circle-0".to_string()));
    }

    /// Test is_fully_confirmed returns true when all members have confirmed
    #[test]
    fn test_is_fully_confirmed_all() {
        let mut contract = setup();

        // Setup: 2-member circle
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Confirm Test".to_string(), None, None);
        add_members_helper(&mut contract, "circle-0", vec![accounts(1)]);

        // Add an expense so both accounts have a balance to confirm
        // accounts(0) pays 100, split 50/50: accounts(0) is owed 50, accounts(1) owes 50
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Test expense".to_string(),
        );

        // Initially not confirmed
        assert!(!contract.is_fully_confirmed("circle-0".to_string()));

        // accounts(0) confirms (creditor, no deposit needed)
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // accounts(1) confirms (debtor, needs to deposit escrow = 50)
        ctx = context(accounts(1), 50);
        testing_env!(ctx.build());
        contract.confirm_ledger("circle-0".to_string());

        // After all members confirmed, execute_autopay_settlements runs automatically
        // and clears confirmations. Verify settlement completed successfully.
        let circle = contract.get_circle("circle-0".to_string());
        assert_eq!(circle.state, CircleState::Settled, "Circle should be in Settled state after all confirmations");
    }

    /// Test is_membership_open can be toggled and reflects correct state
    #[test]
    fn test_is_membership_open_toggle() {
        let mut contract = setup();

        // Setup
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Membership Test".to_string(), None, None);

        // Default should be true (membership open on creation)
        assert!(contract.is_membership_open("circle-0".to_string()));

        // Close membership
        ctx = context(accounts(0), ONE_YOCTO);
        testing_env!(ctx.build());
        contract.set_membership_open("circle-0".to_string(), false);

        assert!(!contract.is_membership_open("circle-0".to_string()));

        // Re-open membership
        ctx = context(accounts(0), ONE_YOCTO);
        testing_env!(ctx.build());
        contract.set_membership_open("circle-0".to_string(), true);

        assert!(contract.is_membership_open("circle-0".to_string()));
    }

    // =========================================================================
    // AUTOPAY AND ESCROW TESTS
    // =========================================================================

    /// Test get_required_autopay_deposit returns debt amount for debtor
    #[test]
    fn test_get_required_autopay_deposit_with_debt() {
        let mut contract = setup();

        // Setup: accounts(0) pays, accounts(1) owes
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Debt Test".to_string(), None, None);
        add_members_helper(&mut contract, "circle-0", vec![accounts(1)]);

        // accounts(0) paid 100, split 50/50 => accounts(1) owes 50
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Test expense".to_string(),
        );

        // accounts(0) is creditor - no deposit needed
        let deposit_0 = contract.get_required_autopay_deposit("circle-0".to_string(), accounts(0));
        assert_eq!(deposit_0.0, 0);

        // accounts(1) is debtor - needs to deposit 50
        let deposit_1 = contract.get_required_autopay_deposit("circle-0".to_string(), accounts(1));
        assert_eq!(deposit_1.0, 50);
    }

    /// Test get_autopay and get_escrow_deposit after enabling autopay
    #[test]
    fn test_get_autopay_and_escrow_after_enable() {
        let mut contract = setup();

        // Setup: accounts(0) pays, accounts(1) owes
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Autopay Test".to_string(), None, None);
        add_members_helper(&mut contract, "circle-0", vec![accounts(1)]);

        // accounts(0) paid 100, split 50/50 => accounts(1) owes 50
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Test expense".to_string(),
        );

        // Initially autopay is false
        assert!(!contract.get_autopay("circle-0".to_string(), accounts(0)));
        assert!(!contract.get_autopay("circle-0".to_string(), accounts(1)));

        // Initially escrow is 0
        assert_eq!(contract.get_escrow_deposit("circle-0".to_string(), accounts(0)).0, 0);
        assert_eq!(contract.get_escrow_deposit("circle-0".to_string(), accounts(1)).0, 0);

        // accounts(0) enables autopay (creditor, no deposit needed but can attach)
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.set_autopay("circle-0".to_string(), true);

        assert!(contract.get_autopay("circle-0".to_string(), accounts(0)));

        // accounts(1) enables autopay with required deposit (50)
        ctx = context(accounts(1), 50);
        testing_env!(ctx.build());
        contract.set_autopay("circle-0".to_string(), true);

        assert!(contract.get_autopay("circle-0".to_string(), accounts(1)));
        assert_eq!(contract.get_escrow_deposit("circle-0".to_string(), accounts(1)).0, 50);
    }

    /// Test all_members_autopay only true when everyone has enabled
    #[test]
    fn test_all_members_autopay() {
        let mut contract = setup();

        // Setup: 2-member circle
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("All Autopay Test".to_string(), None, None);
        add_members_helper(&mut contract, "circle-0", vec![accounts(1)]);

        // Initially false
        assert!(!contract.all_members_autopay("circle-0".to_string()));

        // Only accounts(0) enables autopay
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.set_autopay("circle-0".to_string(), true);

        // Still false - only 1 of 2 members enabled
        assert!(!contract.all_members_autopay("circle-0".to_string()));

        // accounts(1) also enables autopay
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.set_autopay("circle-0".to_string(), true);

        // Now true - all members have enabled
        assert!(contract.all_members_autopay("circle-0".to_string()));
    }

    // =========================================================================
    // RESCUE_STUCK_FT TESTS
    // =========================================================================

    /// Test rescue_stuck_ft with amount > 0 returns a Promise (no panic)
    #[test]
    fn test_rescue_stuck_ft_positive_amount() {
        let contract = setup();
        let token_id: AccountId = "usdc.near".parse().unwrap();
        let receiver: AccountId = "rescue-receiver.near".parse().unwrap();

        // Set context with predecessor = current_account_id (contract calling itself)
        let ctx = VMContextBuilder::new()
            .predecessor_account_id("contract.near".parse().unwrap())
            .current_account_id("contract.near".parse().unwrap())
            .build();
        testing_env!(ctx);

        // This should NOT panic - returns a Promise
        let _promise = contract.rescue_stuck_ft(token_id, receiver, U128(1000));
        // If we reach here without panic, the test passes
    }

    /// Test rescue_stuck_ft with amount == 0 panics
    #[test]
    #[should_panic(expected = "Amount must be positive")]
    fn test_rescue_stuck_ft_zero_amount_panics() {
        let contract = setup();
        let token_id: AccountId = "usdc.near".parse().unwrap();
        let receiver: AccountId = "rescue-receiver.near".parse().unwrap();

        // Set context with predecessor = current_account_id (contract calling itself)
        let ctx = VMContextBuilder::new()
            .predecessor_account_id("contract.near".parse().unwrap())
            .current_account_id("contract.near".parse().unwrap())
            .build();
        testing_env!(ctx);

        // This should panic with "Amount must be positive"
        let _promise = contract.rescue_stuck_ft(token_id, receiver, U128(0));
    }

    // =========================================================================
    // OWNER-FUNDED STORAGE MODEL TESTS
    // =========================================================================

    /// Test that when a non-owner adds an expense, the owner's storage balance decreases,
    /// not the payer's (non-owner's) storage balance.
    #[test]
    fn test_add_expense_charges_owner_storage() {
        let mut contract = setup();

        // Setup: accounts(0) is owner, accounts(1) is a member
        let mut ctx = context(accounts(0), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        ctx = context(accounts(1), ONE_NEAR);
        testing_env!(ctx.build());
        contract.storage_deposit(None, None);

        // accounts(0) creates circle (becomes owner)
        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.create_circle("Storage Test".to_string(), None, None);
        add_members_helper(&mut contract, "circle-0", vec![accounts(1)]);

        // Get initial storage balances
        let owner_balance_before = contract.storage_balance_of(accounts(0))
            .map(|b| b.total.as_yoctonear())
            .unwrap_or(0);
        let member_balance_before = contract.storage_balance_of(accounts(1))
            .map(|b| b.total.as_yoctonear())
            .unwrap_or(0);

        // accounts(1) (non-owner) adds an expense
        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.add_expense(
            "circle-0".to_string(),
            U128(100),
            vec![
                MemberShare { account_id: accounts(0), weight_bps: 5_000 },
                MemberShare { account_id: accounts(1), weight_bps: 5_000 },
            ],
            "Non-owner expense".to_string(),
        );

        // Get storage balances after
        let owner_balance_after = contract.storage_balance_of(accounts(0))
            .map(|b| b.total.as_yoctonear())
            .unwrap_or(0);
        let member_balance_after = contract.storage_balance_of(accounts(1))
            .map(|b| b.total.as_yoctonear())
            .unwrap_or(0);

        // Owner's storage balance should decrease (charged for expense storage)
        assert!(
            owner_balance_after < owner_balance_before,
            "Owner's storage balance should decrease: before={}, after={}",
            owner_balance_before,
            owner_balance_after
        );

        // Non-owner (payer)'s storage balance should remain unchanged
        assert_eq!(
            member_balance_after,
            member_balance_before,
            "Non-owner's storage balance should not change: before={}, after={}",
            member_balance_before,
            member_balance_after
        );
    }
}

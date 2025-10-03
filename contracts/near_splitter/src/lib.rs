use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;

use near_contract_standards::fungible_token::metadata::FungibleTokenMetadata;
use near_contract_standards::storage_management::{StorageBalance, StorageBalanceBounds};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap};
use near_sdk::env;
use near_sdk::json_types::{I128, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::serde_json::{self, json};
use near_sdk::{
    assert_self, ext_contract, near_bindgen, require, AccountId, BorshStorageKey, Gas, NearToken,
    PanicOnDefault, Promise, PromiseOrValue, PromiseResult,
};

const STORAGE_BYTES_PER_ACCOUNT: u64 = 2_500;
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

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
    Circles,
    Expenses,
    Settlements,
    CirclesByOwner,
    StorageDeposits,
    MetadataCache,
    Confirmations,
    AutopayPreferences,
    EscrowDeposits,
    LockedCircles,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Circle {
    pub id: String,
    pub owner: AccountId,
    pub name: String,
    pub members: Vec<AccountId>,
    pub created_ms: u64,
    /// Optional invite code hash for private circles. If set, users must provide the code to join.
    pub invite_code_hash: Option<String>,
    /// When true, no new expenses can be added (settlement in progress)
    pub locked: bool,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct MemberShare {
    pub account_id: AccountId,
    pub weight_bps: u16,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
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
pub struct NearSplitter {
    circles: UnorderedMap<String, Circle>,
    expenses: LookupMap<String, Vec<Expense>>,
    settlements: LookupMap<String, Vec<Settlement>>,
    circles_by_owner: LookupMap<AccountId, Vec<String>>,
    storage_deposits: LookupMap<AccountId, u128>,
    metadata_cache: LookupMap<AccountId, FungibleTokenMetadata>,
    next_circle_index: u64,
    /// Tracks which members have confirmed the ledger for each circle
    /// Key: circle_id, Value: Vec of account_ids who confirmed
    confirmations: LookupMap<String, Vec<AccountId>>,
    /// Tracks autopay preference for each user in each circle
    /// Key: "circle_id:account_id", Value: true if autopay enabled
    autopay_preferences: LookupMap<String, bool>,
    /// Tracks escrowed NEAR deposits for autopay settlements
    /// Key: "circle_id:account_id", Value: amount in yoctoNEAR
    escrow_deposits: LookupMap<String, u128>,
}

#[near_bindgen]
impl NearSplitter {
    #[init]
    pub fn new() -> Self {
        Self {
            circles: UnorderedMap::new(StorageKey::Circles),
            expenses: LookupMap::new(StorageKey::Expenses),
            settlements: LookupMap::new(StorageKey::Settlements),
            circles_by_owner: LookupMap::new(StorageKey::CirclesByOwner),
            storage_deposits: LookupMap::new(StorageKey::StorageDeposits),
            metadata_cache: LookupMap::new(StorageKey::MetadataCache),
            next_circle_index: 0,
            confirmations: LookupMap::new(StorageKey::Confirmations),
            autopay_preferences: LookupMap::new(StorageKey::AutopayPreferences),
            escrow_deposits: LookupMap::new(StorageKey::EscrowDeposits),
        }
    }

    /// Migration method to add new fields to existing contract
    #[init(ignore_state)]
    #[private]
    pub fn migrate() -> Self {
        #[derive(BorshDeserialize, BorshSerialize)]
        struct OldCircle {
            id: String,
            owner: AccountId,
            name: String,
            members: Vec<AccountId>,
            created_ms: u64,
            invite_code_hash: Option<String>,
        }

        #[derive(BorshDeserialize)]
        struct OldNearSplitter {
            circles: UnorderedMap<String, OldCircle>,
            expenses: LookupMap<String, Vec<Expense>>,
            settlements: LookupMap<String, Vec<Settlement>>,
            circles_by_owner: LookupMap<AccountId, Vec<String>>,
            storage_deposits: LookupMap<AccountId, u128>,
            metadata_cache: LookupMap<AccountId, FungibleTokenMetadata>,
            next_circle_index: u64,
            confirmations: LookupMap<String, Vec<AccountId>>,
        }

        let old: OldNearSplitter = env::state_read().expect("Failed to read state");
        
        // Convert old circles to new circles with locked field
        let mut new_circles: UnorderedMap<String, Circle> = UnorderedMap::new(StorageKey::Circles);
        for (circle_id, old_circle) in old.circles.iter() {
            let new_circle = Circle {
                id: old_circle.id,
                owner: old_circle.owner,
                name: old_circle.name,
                members: old_circle.members,
                created_ms: old_circle.created_ms,
                invite_code_hash: old_circle.invite_code_hash,
                locked: false, // New field, default to false
            };
            new_circles.insert(&circle_id, &new_circle);
        }
        
        Self {
            circles: new_circles,
            expenses: old.expenses,
            settlements: old.settlements,
            circles_by_owner: old.circles_by_owner,
            storage_deposits: old.storage_deposits,
            metadata_cache: old.metadata_cache,
            next_circle_index: old.next_circle_index,
            confirmations: old.confirmations,
            autopay_preferences: LookupMap::new(StorageKey::AutopayPreferences),
            escrow_deposits: LookupMap::new(StorageKey::EscrowDeposits),
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
        let slice = paginate_vec(&circles, from.unwrap_or(0), limit.unwrap_or(50));
        slice
            .iter()
            .filter_map(|id| self.circles.get(id))
            .collect()
    }

    /// Get all circles where the given account is a member (including owned circles)
    pub fn list_circles_by_member(
        &self,
        account_id: AccountId,
        from: Option<u64>,
        limit: Option<u64>,
    ) -> Vec<Circle> {
        let from_index = from.unwrap_or(0) as usize;
        let limit_count = limit.unwrap_or(50) as usize;
        
        // Iterate through all circles and find ones where the account is a member
        let member_circles: Vec<Circle> = self
            .circles
            .keys()
            .skip(from_index)
            .take(limit_count * 2) // Take extra to account for filtering
            .filter_map(|circle_id| {
                self.circles.get(&circle_id).and_then(|circle| {
                    if circle.members.contains(&account_id) {
                        Some(circle)
                    } else {
                        None
                    }
                })
            })
            .take(limit_count) // Apply final limit after filtering
            .collect();
        
        member_circles
    }

    pub fn list_expenses(
        &self,
        circle_id: String,
        from: Option<u64>,
        limit: Option<u64>,
    ) -> Vec<Expense> {
        let expenses = self.expenses.get(&circle_id).unwrap_or_default();
        paginate_vec(&expenses, from.unwrap_or(0), limit.unwrap_or(50))
    }

    pub fn compute_balances(&self, circle_id: String) -> Vec<BalanceView> {
        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));
        let expenses = self.expenses.get(&circle_id).unwrap_or_default();

        let mut net_map: HashMap<AccountId, i128> = HashMap::new();
        for member in &circle.members {
            net_map.entry(member.clone()).or_insert(0);
        }

        for expense in expenses {
            let payer = &expense.payer;
            let amount_u128 = expense.amount_yocto.0;
            let amount_i128 = i128::try_from(amount_u128).expect("Amount exceeds i128 range");

            let mut remaining = amount_u128;
            let last_index = expense.participants.len().saturating_sub(1);

            for (idx, share) in expense.participants.iter().enumerate() {
                let share_amount_u128 = if idx == last_index {
                    remaining
                } else {
                    let computed = amount_u128
                        .checked_mul(share.weight_bps as u128)
                        .expect("Share multiplication overflow")
                        / TARGET_BPS_TOTAL as u128;
                    remaining = remaining
                        .checked_sub(computed)
                        .expect("Share subtraction underflow");
                    computed
                };

                let share_i128 = i128::try_from(share_amount_u128).expect("Share exceeds i128");
                let entry = net_map.entry(share.account_id.clone()).or_insert(0);
                *entry -= share_i128;
            }

            let payer_entry = net_map.entry(payer.clone()).or_insert(0);
            *payer_entry += amount_i128;
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

    pub fn suggest_settlements(&self, circle_id: String) -> Vec<SettlementSuggestion> {
        let balances = self.compute_balances(circle_id);
        let mut debtors: Vec<(AccountId, u128)> = Vec::new();
        let mut creditors: Vec<(AccountId, u128)> = Vec::new();

        for balance in balances {
            match balance.net.0.cmp(&0) {
                Ordering::Less => debtors.push((balance.account_id, balance.net.0.unsigned_abs())),
                Ordering::Greater => {
                    let credit = u128::try_from(balance.net.0).expect("Positive balance overflow");
                    creditors.push((balance.account_id, credit));
                }
                Ordering::Equal => {}
            }
        }

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

    pub fn create_circle(&mut self, name: String, invite_code: Option<String>) -> String {
        let owner = env::predecessor_account_id();
        self.assert_registered(&owner);
        require!(!name.trim().is_empty(), "Circle name cannot be empty");

        let circle_id = format!("circle-{}", self.next_circle_index);
        self.next_circle_index += 1;
        let created_ms = timestamp_ms();

        let mut members = Vec::new();
        members.push(owner.clone());

        // Hash the invite code if provided for security
        let invite_code_hash = invite_code.map(|code| {
            require!(!code.trim().is_empty(), "Invite code cannot be empty");
            env::sha256(code.as_bytes())
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>()
        });

        let circle = Circle {
            id: circle_id.clone(),
            owner: owner.clone(),
            name: name.clone(),
            members,
            created_ms,
            invite_code_hash,
            locked: false,
        };

        self.circles.insert(&circle_id, &circle);

        let mut owner_list = self.circles_by_owner.get(&owner).unwrap_or_default();
        owner_list.push(circle_id.clone());
        self.circles_by_owner.insert(&owner, &owner_list);

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

    pub fn join_circle(&mut self, circle_id: String, invite_code: Option<String>) {
        let account = env::predecessor_account_id();
        self.assert_registered(&account);

        let mut circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        // Verify invite code if circle is private
        if let Some(expected_hash) = &circle.invite_code_hash {
            let provided_code = invite_code.unwrap_or_else(|| env::panic_str("This circle requires an invite code"));
            let provided_hash = env::sha256(provided_code.as_bytes())
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>();
            require!(
                &provided_hash == expected_hash,
                "Invalid invite code"
            );
        }

        require!(circle.members.len() < 256, "Member cap reached");
        require!(circle.members.iter().all(|m| m != &account), "Already a member");

        circle.members.push(account.clone());
        self.circles.insert(&circle_id, &circle);

        self.emit_event(
            "circle_join",
            json!([{ "circle_id": circle_id, "account_id": account }]),
        );
    }

    pub fn add_expense(
        &mut self,
        circle_id: String,
        amount_yocto: U128,
        shares: Vec<MemberShare>,
        memo: String,
    ) {
        require!(amount_yocto.0 > 0, "Amount must be positive");
        require!(!shares.is_empty(), "At least one share is required");

        let payer = env::predecessor_account_id();
        self.assert_registered(&payer);

        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));
        
        require!(!circle.locked, "Circle is locked for settlement. Cannot add expenses.");
        
        require!(
            circle.members.iter().any(|m| m == &payer),
            "Payer must be circle member",
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

        let mut expenses = self.expenses.get(&circle_id).unwrap_or_else(Vec::new);
        let expense_id = format!("expense-{}-{}", circle_id, expenses.len() + 1);
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

        expenses.push(expense);
        self.expenses.insert(&circle_id, &expenses);

        // Reset confirmations when new expense is added
        self.confirmations.remove(&circle_id);

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

    #[payable]
    pub fn pay_native(&mut self, circle_id: String, to: AccountId) {
        let payer = env::predecessor_account_id();
        let amount = env::attached_deposit().as_yoctonear();
        require!(amount > 0, "Attach deposit equal to settlement amount");

        self.assert_registered(&payer);
        self.assert_registered(&to);

        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));
        require!(circle.members.iter().any(|m| m == &payer), "Payer must be member");
        require!(circle.members.iter().any(|m| m == &to), "Recipient must be member");

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

        Promise::new(to).transfer(yocto_to_token(amount));
    }

    pub fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<String> {
        require!(
            env::attached_deposit().as_yoctonear() == 0,
            "No deposit expected",
        );
        require!(amount.0 > 0, "Amount must be positive");
        let token_contract = env::predecessor_account_id();
        let payload: TransferMessage =
            serde_json::from_str(&msg).unwrap_or_else(|_| env::panic_str("Invalid message"));

        let circle = self
            .circles
            .get(&payload.circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));
        require!(
            circle.members.iter().any(|m| m == &sender_id),
            "Sender must be member",
        );
        require!(
            circle.members.iter().any(|m| m == &payload.to),
            "Recipient must be member",
        );

        self.assert_registered(&sender_id);
        self.assert_registered(&payload.to);

        let promise = ext_ft::ext(token_contract.clone())
            .with_attached_deposit(yocto_to_token(ONE_YOCTO))
            .with_static_gas(gas_ft_transfer())
            .ft_transfer(payload.to.clone(), amount, None)
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(gas_ft_callback())
                    .on_ft_transfer_settlement(
                        payload.circle_id.clone(),
                        sender_id.clone(),
                        payload.to.clone(),
                        amount,
                        token_contract.clone(),
                    ),
            );

        PromiseOrValue::Promise(promise)
    }

    pub fn ft_metadata(&self, token_id: AccountId) -> Option<FungibleTokenMetadata> {
        self.metadata_cache.get(&token_id)
    }

    pub fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        let cost = self.required_storage_cost();
        StorageBalanceBounds {
            min: yocto_to_token(cost),
            max: Some(yocto_to_token(cost)),
        }
    }

    #[payable]
    pub fn storage_deposit(
        &mut self,
        account_id: Option<AccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
    let account_id = account_id.unwrap_or_else(|| env::predecessor_account_id());
    let deposit = env::attached_deposit().as_yoctonear();
        let cost = self.required_storage_cost();

        if let Some(balance) = self.storage_deposits.get(&account_id) {
            if let Some(true) = registration_only {
                require!(deposit == 0, "Registration only deposit must be zero");
            }
            if deposit > 0 {
                Promise::new(env::predecessor_account_id())
                    .transfer(yocto_to_token(deposit));
            }

            let available = balance.saturating_sub(cost);
            return StorageBalance {
                total: yocto_to_token(balance),
                available: yocto_to_token(available),
            };
        }

        require!(deposit >= cost, "Insufficient deposit");
        self.storage_deposits.insert(&account_id, &cost);
        if deposit > cost {
            Promise::new(env::predecessor_account_id())
                .transfer(yocto_to_token(deposit - cost));
        }

        StorageBalance {
            total: yocto_to_token(cost),
            available: yocto_to_token(0),
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

        if !can_force {
            require!(
                !self.is_member_any_circle(&account),
                "Remove account from circles before unregistering",
            );
        }

        if let Some(balance) = self.storage_deposits.remove(&account) {
            Promise::new(account.clone()).transfer(yocto_to_token(balance));
            self.emit_event("storage_unregister", json!([{ "account_id": account }]));
            true
        } else {
            false
        }
    }

    #[payable]
    pub fn cache_ft_metadata(&mut self, token_id: AccountId, metadata: FungibleTokenMetadata) {
        require!(
            env::attached_deposit().as_yoctonear() == ONE_YOCTO,
            "Attach 1 yoctoNEAR to cache metadata",
        );
        self.metadata_cache.insert(&token_id, &metadata);
    }

    fn required_storage_cost(&self) -> u128 {
        env::storage_byte_cost().as_yoctonear() * (STORAGE_BYTES_PER_ACCOUNT as u128)
    }

    fn record_settlement(&mut self, settlement: Settlement) {
        let circle_id = settlement.circle_id.clone();
        let mut list = self.settlements.get(&circle_id).unwrap_or_else(Vec::new);

        let event_payload = json!([{
            "circle_id": settlement.circle_id.clone(),
            "from": settlement.from.clone(),
            "to": settlement.to.clone(),
            "amount": settlement.amount,
            "token": settlement.token.clone(),
            "tx_kind": settlement.tx_kind.clone(),
            "ts_ms": settlement.ts_ms,
        }]);

        list.push(settlement);
        self.settlements.insert(&circle_id, &list);

        self.emit_event("settlement_paid", event_payload);
    }

    fn assert_registered(&self, account_id: &AccountId) {
        require!(
            self.storage_deposits.get(account_id).is_some(),
            "Account must call storage_deposit first",
        );
    }

    fn is_member_any_circle(&self, account_id: &AccountId) -> bool {
        self.circles
            .iter()
            .any(|(_, circle)| circle.members.iter().any(|m| m == account_id))
    }

    fn emit_event(&self, event: &str, data: serde_json::Value) {
        let payload = json!({
            "standard": EVENT_STANDARD,
            "version": EVENT_VERSION,
            "event": event,
            "data": data,
        });
        env::log_str(&format!("EVENT_JSON:{}", payload.to_string()));
    }

    #[private]
    pub fn on_ft_transfer_settlement(
        &mut self,
        circle_id: String,
        sender_id: AccountId,
        to: AccountId,
        amount: U128,
        token_id: AccountId,
    ) -> String {
        assert_self();
        match env::promise_result(0) {
            PromiseResult::Successful(_) => {
                let settlement = Settlement {
                    circle_id,
                    from: sender_id,
                    to,
                    amount,
                    token: Some(token_id),
                    ts_ms: timestamp_ms(),
                    tx_kind: "ft_transfer".to_string(),
                };
                self.record_settlement(settlement);
                "0".to_string()
            }
            _ => amount.0.to_string(),
        }
    }
}

#[ext_contract(ext_ft)]
pub trait ExtFungibleToken {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
}

#[ext_contract(ext_self)]
pub trait ExtSelf {
    fn on_ft_transfer_settlement(
        &mut self,
        circle_id: String,
        sender_id: AccountId,
        to: AccountId,
        amount: U128,
        token_id: AccountId,
    ) -> String;
}

#[near_bindgen]
impl NearSplitter {
    /// Confirm the ledger for a circle. Once all members confirm, settlement can proceed.
    /// First confirmation locks the circle (no new expenses). 
    /// If all members have autopay enabled, automatically distributes escrowed funds.
    /// Confirm the ledger for a circle. 
    /// This automatically enables autopay and requires escrow deposit if user has debt.
    /// Once all members confirm, settlement proceeds automatically.
    #[payable]
    pub fn confirm_ledger(&mut self, circle_id: String) -> Promise {
        let account = env::predecessor_account_id();
        let deposit = env::attached_deposit().as_yoctonear();
        self.assert_registered(&account);

        let mut circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(
            circle.members.iter().any(|m| m == &account),
            "Only circle members can confirm"
        );

        let mut confirmations = self.confirmations.get(&circle_id).unwrap_or_default();
        
        require!(
            !confirmations.iter().any(|c| c == &account),
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
                &format!("Must deposit {} yoctoNEAR to cover your debt of {} yoctoNEAR", deposit, debt)
            );

            // Store the deposit in escrow
            let escrow_key = format!("{}:{}", circle_id, account);
            let existing_deposit = self.escrow_deposits.get(&escrow_key).unwrap_or(0);
            self.escrow_deposits.insert(&escrow_key, &(existing_deposit + deposit));

            self.emit_event(
                "escrow_deposited",
                json!({
                    "circle_id": circle_id.clone(),
                    "account_id": account.clone(),
                    "amount": U128(deposit),
                    "total_escrowed": U128(existing_deposit + deposit),
                }),
            );
        } else if deposit > 0 {
            // User is creditor or even, but deposited anyway - refund
            Promise::new(account.clone()).transfer(yocto_to_token(deposit));
        }

        // Automatically enable autopay for this user
        let autopay_key = format!("{}:{}", circle_id, account);
        self.autopay_preferences.insert(&autopay_key, &true);

        self.emit_event(
            "autopay_enabled",
            json!({
                "circle_id": circle_id.clone(),
                "account_id": account.clone(),
            }),
        );

        // Lock the circle on first confirmation
        if confirmations.is_empty() && !circle.locked {
            circle.locked = true;
            self.circles.insert(&circle_id, &circle);
            
            self.emit_event(
                "circle_locked",
                json!({
                    "circle_id": circle_id.clone(),
                    "message": "Circle locked for settlement. No new expenses allowed.",
                }),
            );
        }

        confirmations.push(account.clone());
        self.confirmations.insert(&circle_id, &confirmations);

        self.emit_event(
            "ledger_confirmed",
            json!({
                "circle_id": circle_id.clone(),
                "account_id": account,
                "confirmations": confirmations.len(),
                "total_members": circle.members.len(),
            }),
        );

        // If all members confirmed, execute autopay settlements
        if confirmations.len() == circle.members.len() {
            self.execute_autopay_settlements(circle_id)
        } else {
            // Not all confirmed yet, return no-op promise
            Promise::new(env::current_account_id())
        }
    }

    /// Execute autopay settlements when all members have confirmed
    /// Returns a Promise that must be returned from the calling public method
    fn execute_autopay_settlements(&mut self, circle_id: String) -> Promise {
        let circle = self.circles.get(&circle_id).expect("Circle not found");
        
        // Get settlement suggestions
        let suggestions = self.suggest_settlements(circle_id.clone());
        
        // Determine which members have autopay enabled
        let autopay_members: Vec<AccountId> = circle.members.iter()
            .filter(|member| {
                let key = format!("{}:{}", circle_id, member);
                self.autopay_preferences.get(&key).unwrap_or(false)
            })
            .cloned()
            .collect();

        let all_autopay = autopay_members.len() == circle.members.len();
        
        // Collect transfers to make at the end
        let mut transfers_to_make: Vec<(AccountId, u128)> = Vec::new();

        if all_autopay {
            // All members have autopay - distribute escrowed funds
            self.emit_event(
                "autopay_triggered",
                json!({
                    "circle_id": circle_id,
                    "message": "All members have autopay. Distributing escrowed funds.",
                    "settlement_count": suggestions.len(),
                    "autopay_members": autopay_members.len(),
                }),
            );

            // Execute actual transfers from escrow
            for suggestion in &suggestions {
                if suggestion.amount.0 > 0 {
                    let from_key = format!("{}:{}", circle_id, suggestion.from);
                    let escrowed = self.escrow_deposits.get(&from_key).unwrap_or(0);
                    
                    self.emit_event(
                        "settlement_processing",
                        json!({
                            "circle_id": circle_id,
                            "from": suggestion.from,
                            "to": suggestion.to,
                            "amount": suggestion.amount,
                            "escrowed": U128(escrowed),
                        }),
                    );
                    
                    if escrowed >= suggestion.amount.0 {
                        // Deduct from escrow
                        let remaining = escrowed - suggestion.amount.0;
                        if remaining > 0 {
                            self.escrow_deposits.insert(&from_key, &remaining);
                        } else {
                            self.escrow_deposits.remove(&from_key);
                        }

                        // Queue the transfer (don't execute yet)
                        transfers_to_make.push((suggestion.to.clone(), suggestion.amount.0));

                        // Record settlement
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
                            json!({
                                "circle_id": circle_id,
                                "from": suggestion.from,
                                "to": suggestion.to,
                                "amount": suggestion.amount,
                            }),
                        );
                    } else {
                        self.emit_event(
                            "settlement_failed",
                            json!({
                                "circle_id": circle_id,
                                "from": suggestion.from,
                                "to": suggestion.to,
                                "amount": suggestion.amount,
                                "escrowed": U128(escrowed),
                                "reason": "Insufficient escrow",
                            }),
                        );
                    }
                }
            }

            // Refund any remaining escrow to members
            for member in &circle.members {
                let escrow_key = format!("{}:{}", circle_id, member);
                if let Some(remaining) = self.escrow_deposits.get(&escrow_key) {
                    if remaining > 0 {
                        self.escrow_deposits.remove(&escrow_key);
                        // Queue refund transfer
                        transfers_to_make.push((member.clone(), remaining));
                    }
                }
            }

        } else {
            // Partial autopay - optimize settlements
            self.emit_event(
                "partial_autopay",
                json!({
                    "circle_id": circle_id,
                    "autopay_members": autopay_members.len(),
                    "total_members": circle.members.len(),
                }),
            );

            // Execute transfers for autopay members only
            for suggestion in &suggestions {
                let from_autopay = autopay_members.contains(&suggestion.from);
                let to_autopay = autopay_members.contains(&suggestion.to);

                if from_autopay && suggestion.amount.0 > 0 {
                    // Debtor has autopay - use escrow
                    let from_key = format!("{}:{}", circle_id, suggestion.from);
                    let escrowed = self.escrow_deposits.get(&from_key).unwrap_or(0);
                    
                    if escrowed >= suggestion.amount.0 {
                        // Deduct from escrow
                        let remaining = escrowed - suggestion.amount.0;
                        if remaining > 0 {
                            self.escrow_deposits.insert(&from_key, &remaining);
                        } else {
                            self.escrow_deposits.remove(&from_key);
                        }

                        // Queue transfer
                        transfers_to_make.push((suggestion.to.clone(), suggestion.amount.0));

                        // Record settlement
                        let settlement = Settlement {
                            circle_id: circle_id.clone(),
                            from: suggestion.from.clone(),
                            to: suggestion.to.clone(),
                            amount: suggestion.amount,
                            token: None,
                            ts_ms: timestamp_ms(),
                            tx_kind: if to_autopay { "autopay_escrow" } else { "autopay_manual_recipient" }.to_string(),
                        };
                        self.record_settlement(settlement);
                    }
                }
            }
        }

        // Clear expenses and confirmations (do this BEFORE returning promises)
        self.expenses.remove(&circle_id);
        self.confirmations.remove(&circle_id);
        
        // Unlock circle for new expenses
        let mut updated_circle = circle.clone();
        updated_circle.locked = false;
        self.circles.insert(&circle_id, &updated_circle);

        self.emit_event(
            "ledger_settled",
            json!({
                "circle_id": circle_id,
                "all_autopay": all_autopay,
            }),
        );

        // NOW create and return all promises batched together
        if transfers_to_make.is_empty() {
            // No transfers needed
            Promise::new(env::current_account_id())
        } else {
            // Create first promise
            let (first_recipient, first_amount) = &transfers_to_make[0];
            let mut batch = Promise::new(first_recipient.clone()).transfer(yocto_to_token(*first_amount));
            
            // Chain remaining promises
            for (recipient, amount) in transfers_to_make.iter().skip(1) {
                batch = batch.and(Promise::new(recipient.clone()).transfer(yocto_to_token(*amount)));
            }
            
            batch
        }
    }

    /// Get the list of accounts that have confirmed the ledger for a circle
    pub fn get_confirmations(&self, circle_id: String) -> Vec<AccountId> {
        self.confirmations.get(&circle_id).unwrap_or_default()
    }

    /// Check if all members have confirmed the ledger
    pub fn is_fully_confirmed(&self, circle_id: String) -> bool {
        let circle = self.circles.get(&circle_id);
        if circle.is_none() {
            return false;
        }
        let circle = circle.unwrap();
        let confirmations = self.confirmations.get(&circle_id).unwrap_or_default();
        confirmations.len() == circle.members.len()
    }

    /// Reset confirmations for a circle (e.g., after adding new expenses)
    pub fn reset_confirmations(&mut self, circle_id: String) {
        let account = env::predecessor_account_id();
        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

        require!(circle.owner == account, "Only circle owner can reset confirmations");

        self.confirmations.remove(&circle_id);
        
        self.emit_event(
            "confirmations_reset",
            json!({
                "circle_id": circle_id,
            }),
        );
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

        let key = format!("{}:{}", circle_id, account);

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

                // Store the deposit in escrow
                let escrow_key = format!("{}:{}", circle_id, account);
                let existing_deposit = self.escrow_deposits.get(&escrow_key).unwrap_or(0);
                self.escrow_deposits.insert(&escrow_key, &(existing_deposit + deposit));

                self.emit_event(
                    "escrow_deposited",
                    json!({
                        "circle_id": circle_id,
                        "account_id": account,
                        "amount": U128(deposit),
                        "total_escrowed": U128(existing_deposit + deposit),
                    }),
                );
            } else if deposit > 0 {
                // User is creditor or even, but deposited anyway - refund
                Promise::new(account.clone()).transfer(yocto_to_token(deposit));
            }
        } else {
            // Disabling autopay - refund any escrowed funds
            let escrow_key = format!("{}:{}", circle_id, account);
            if let Some(escrowed_amount) = self.escrow_deposits.get(&escrow_key) {
                if escrowed_amount > 0 {
                    self.escrow_deposits.remove(&escrow_key);
                    Promise::new(account.clone()).transfer(yocto_to_token(escrowed_amount));
                    
                    self.emit_event(
                        "escrow_refunded",
                        json!({
                            "circle_id": circle_id,
                            "account_id": account,
                            "amount": U128(escrowed_amount),
                        }),
                    );
                }
            }
        }

        self.autopay_preferences.insert(&key, &enabled);

        self.emit_event(
            "autopay_preference_set",
            json!({
                "circle_id": circle_id,
                "account_id": account,
                "enabled": enabled,
            }),
        );
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
}

fn paginate_vec<T: Clone>(items: &[T], from: u64, limit: u64) -> Vec<T> {
    if items.is_empty() {
        return Vec::new();
    }
    let start = from.min(items.len() as u64) as usize;
    let end = (start + limit as usize).min(items.len());
    items[start..end].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;

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
        let id = contract.create_circle("Friends".to_string());
        assert_eq!(id, "circle-0");
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
        contract.create_circle("Trip".to_string());

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string());

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
        contract.create_circle("Trip".to_string());

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string());

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
        contract.create_circle("Trip".to_string());

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string());

        ctx = context(accounts(0), 500);
        testing_env!(ctx.build());
        contract.pay_native("circle-0".to_string(), accounts(1));

        let settlements = contract
            .settlements
            .get(&"circle-0".to_string())
            .expect("Settlement recorded");
        assert_eq!(settlements.len(), 1);
        assert_eq!(settlements[0].amount, U128(500));
        assert_eq!(settlements[0].tx_kind, "native");
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
        contract.create_circle("Trip".to_string());

        ctx = context(accounts(1), 0);
        testing_env!(ctx.build());
        contract.join_circle("circle-0".to_string());

        ctx = context(accounts(0), 0);
        testing_env!(ctx.build());
        contract.pay_native("circle-0".to_string(), accounts(1));
    }
}

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
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Circle {
    pub id: String,
    pub owner: AccountId,
    pub name: String,
    pub members: Vec<AccountId>,
    pub created_ms: u64,
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

    pub fn create_circle(&mut self, name: String) -> String {
        let owner = env::predecessor_account_id();
        self.assert_registered(&owner);
        require!(!name.trim().is_empty(), "Circle name cannot be empty");

        let circle_id = format!("circle-{}", self.next_circle_index);
        self.next_circle_index += 1;
        let created_ms = timestamp_ms();

        let mut members = Vec::new();
        members.push(owner.clone());

        let circle = Circle {
            id: circle_id.clone(),
            owner: owner.clone(),
            name: name.clone(),
            members,
            created_ms,
        };

        self.circles.insert(&circle_id, &circle);

        let mut owner_list = self.circles_by_owner.get(&owner).unwrap_or_default();
        owner_list.push(circle_id.clone());
        self.circles_by_owner.insert(&owner, &owner_list);

        self.emit_event(
            "circle_create",
            json!([{ "circle_id": circle_id, "owner": owner, "name": name }]),
        );
        circle.id
    }

    pub fn join_circle(&mut self, circle_id: String) {
        let account = env::predecessor_account_id();
        self.assert_registered(&account);

        let mut circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("Circle not found"));

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

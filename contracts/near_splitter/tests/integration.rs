/*!
 * NearSplitter Integration Tests
 * 
 * These tests run against a local NEAR sandbox to verify the contract
 * behaves correctly in realistic scenarios.
 * 
 * Run with: cargo test --test integration
 */

use near_workspaces::{Account, Contract, DevNetwork, Worker};
use near_workspaces::types::NearToken;
use serde_json::json;

// Use the optimized WASM built with wasm-opt
const WASM_FILEPATH: &str = "./target/wasm32-unknown-unknown/release/near_splitter_optimized.wasm";

/// Helper to deploy and initialize the contract
async fn init_contract(worker: &Worker<impl DevNetwork>) -> anyhow::Result<Contract> {
    let wasm = std::fs::read(WASM_FILEPATH)?;
    let contract = worker.dev_deploy(&wasm).await?;
    
    // Initialize the contract
    contract.call("new").transact().await?.into_result()?;
    
    Ok(contract)
}

/// Helper to get minimum required storage deposit from storage_balance_bounds
async fn storage_min_yocto(contract: &Contract) -> anyhow::Result<u128> {
    let bounds: serde_json::Value = contract
        .view("storage_balance_bounds")
        .args_json(json!({}))
        .await?
        .json()?;
    let min_str = bounds["min"].as_str().ok_or_else(|| anyhow::anyhow!("missing min"))?;
    let min: u128 = min_str.parse()?;
    Ok(min)
}

/// Helper to register an account with the contract (storage deposit)
async fn register_account(contract: &Contract, account: &Account) -> anyhow::Result<()> {
    let min = storage_min_yocto(contract).await?;
    let buffer = NearToken::from_millinear(100).as_yoctonear(); // 0.1 NEAR buffer for operations like add_expense
    account
        .call(contract.id(), "storage_deposit")
        .args_json(json!({}))
        .deposit(NearToken::from_yoctonear(min + buffer))
        .transact()
        .await?
        .into_result()?;
    Ok(())
}

/// Helper to assert that a failure result contains a specific substring
fn assert_failure_contains(res: &near_workspaces::result::ExecutionFinalResult, needle: &str) {
    assert!(res.is_failure(), "Expected failure but got success");
    let debug_str = format!("{res:?}");
    assert!(
        debug_str.contains(needle),
        "Expected failure to contain '{}', got: {}",
        needle,
        debug_str
    );
}

/// Helper to create a circle and return its ID
async fn create_circle(
    contract: &Contract,
    owner: &Account,
    name: &str,
    invite_code: Option<&str>,
) -> anyhow::Result<String> {
    let args = match invite_code {
        Some(code) => json!({ "name": name, "invite_code": code }),
        None => json!({ "name": name }),
    };
    let result = owner
        .call(contract.id(), "create_circle")
        .args_json(args)
        .transact()
        .await?
        .into_result()?;
    let circle_id: String = result.json()?;
    Ok(circle_id)
}

// ============================================================================
// Storage & Registration Tests
// ============================================================================

#[tokio::test]
async fn test_storage_deposit_and_balance() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    
    // Check bounds first
    let bounds: serde_json::Value = contract
        .view("storage_balance_bounds")
        .args_json(json!({}))
        .await?
        .json()?;
    
    assert!(bounds["min"].as_str().is_some(), "Should have min bound");
    
    // Register Alice
    register_account(&contract, &alice).await?;
    
    // Check balance
    let balance: serde_json::Value = contract
        .view("storage_balance_of")
        .args_json(json!({ "account_id": alice.id() }))
        .await?
        .json()?;
    
    assert!(balance["total"].as_str().is_some(), "Should have total balance");
    
    Ok(())
}

#[tokio::test]
async fn test_unregistered_account_cannot_create_circle() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    
    // Try to create circle without registering - should fail
    let result = alice
        .call(contract.id(), "create_circle")
        .args_json(json!({ "name": "Test Circle" }))
        .transact()
        .await?;
    
    assert_failure_contains(&result, "storage_deposit");
    
    Ok(())
}

#[tokio::test]
async fn test_storage_unregister() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    
    // Register
    register_account(&contract, &alice).await?;
    
    // Unregister
    alice
        .call(contract.id(), "storage_unregister")
        .args_json(json!({ "force": true }))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await?
        .into_result()?;
    
    // Verify unregistered
    let balance: Option<serde_json::Value> = contract
        .view("storage_balance_of")
        .args_json(json!({ "account_id": alice.id() }))
        .await?
        .json()?;
    
    assert!(balance.is_none(), "Should be unregistered");
    
    Ok(())
}

#[tokio::test]
async fn test_unregistered_account_cannot_join_circle() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    // Bob is NOT registered
    
    let circle_id = create_circle(&contract, &alice, "Test Circle", None).await?;
    
    // Bob tries to join without registering - should fail
    let result = bob
        .call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": circle_id }))
        .transact()
        .await?;
    
    assert_failure_contains(&result, "storage_deposit");
    
    Ok(())
}

// ============================================================================
// Circle Creation & Management Tests
// ============================================================================

#[tokio::test]
async fn test_create_circle() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    
    // Create a circle
    let result = alice
        .call(contract.id(), "create_circle")
        .args_json(json!({ "name": "Trip to Paris" }))
        .transact()
        .await?
        .into_result()?;
    
    let circle_id: String = result.json()?;
    assert!(circle_id.starts_with("circle-"), "Should return circle ID");
    
    // Verify circle exists
    let circle: serde_json::Value = contract
        .view("get_circle")
        .args_json(json!({ "circle_id": circle_id }))
        .await?
        .json()?;
    
    assert_eq!(circle["name"], "Trip to Paris");
    assert_eq!(circle["owner"].as_str().unwrap(), alice.id().as_str());
    assert_eq!(circle["members"].as_array().unwrap().len(), 1);
    
    Ok(())
}

#[tokio::test]
async fn test_create_private_circle_with_password() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    
    // Create private circle
    let result = alice
        .call(contract.id(), "create_circle")
        .args_json(json!({ 
            "name": "Secret Club",
            "invite_code": "password123"
        }))
        .transact()
        .await?
        .into_result()?;
    
    let circle_id: String = result.json()?;
    
    // Verify it has invite_code_hash
    let circle: serde_json::Value = contract
        .view("get_circle")
        .args_json(json!({ "circle_id": circle_id }))
        .await?
        .json()?;
    
    assert!(circle["invite_code_hash"].as_str().is_some(), "Should have password hash");
    
    Ok(())
}

#[tokio::test]
async fn test_join_circle_public() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    // Alice creates circle
    let circle_id = create_circle(&contract, &alice, "Public Circle", None).await?;
    
    // Bob joins
    bob.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Verify Bob is member
    let circle: serde_json::Value = contract
        .view("get_circle")
        .args_json(json!({ "circle_id": circle_id }))
        .await?
        .json()?;
    
    assert_eq!(circle["members"].as_array().unwrap().len(), 2);
    
    Ok(())
}

#[tokio::test]
async fn test_join_private_circle_wrong_password() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    // Alice creates private circle
    let circle_id = create_circle(&contract, &alice, "Private Circle", Some("correct_password")).await?;
    
    // Bob tries to join with wrong password
    let join_result = bob
        .call(contract.id(), "join_circle")
        .args_json(json!({ 
            "circle_id": circle_id,
            "invite_code": "wrong_password"
        }))
        .transact()
        .await?;
    
    assert_failure_contains(&join_result, "invite code");
    
    Ok(())
}

#[tokio::test]
async fn test_join_private_circle_correct_password() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    // Alice creates private circle
    let circle_id = create_circle(&contract, &alice, "Private Circle", Some("secret123")).await?;
    
    // Bob joins with correct password
    bob.call(contract.id(), "join_circle")
        .args_json(json!({ 
            "circle_id": circle_id,
            "invite_code": "secret123"
        }))
        .transact()
        .await?
        .into_result()?;
    
    // Verify Bob is member
    let circle: serde_json::Value = contract
        .view("get_circle")
        .args_json(json!({ "circle_id": circle_id }))
        .await?
        .json()?;
    
    assert_eq!(circle["members"].as_array().unwrap().len(), 2);
    
    Ok(())
}

#[tokio::test]
async fn test_cannot_join_circle_twice() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    let circle_id = create_circle(&contract, &alice, "Test", None).await?;
    
    // Bob joins first time
    bob.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Bob tries to join again
    let second_join = bob
        .call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": circle_id }))
        .transact()
        .await?;
    
    assert_failure_contains(&second_join, "Already a member");
    
    Ok(())
}

// ============================================================================
// Expense Tests
// ============================================================================

#[tokio::test]
async fn test_add_expense_equal_split() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    // Create circle and have Bob join
    let circle_id = create_circle(&contract, &alice, "Dinner", None).await?;
    
    bob.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Alice pays for dinner, split equally
    let amount = "1000000000000000000000000"; // 1 NEAR in yoctoNEAR
    alice.call(contract.id(), "add_expense")
        .args_json(json!({
            "circle_id": circle_id,
            "amount_yocto": amount,
            "shares": [
                { "account_id": alice.id(), "weight_bps": 5000 },
                { "account_id": bob.id(), "weight_bps": 5000 }
            ],
            "memo": "Dinner at restaurant"
        }))
        .transact()
        .await?
        .into_result()?;
    
    // Check expenses
    let expenses: Vec<serde_json::Value> = contract
        .view("list_expenses")
        .args_json(json!({ "circle_id": circle_id }))
        .await?
        .json()?;
    
    assert_eq!(expenses.len(), 1);
    assert_eq!(expenses[0]["memo"], "Dinner at restaurant");
    
    Ok(())
}

#[tokio::test]
async fn test_autopay_pending_payout_and_withdraw() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;

    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;

    let circle_id = create_circle(&contract, &alice, "Autopay Test", None).await?;

    bob.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": circle_id }))
        .transact()
        .await?
        .into_result()?;

    let amount = "1000000000000000000000000"; // 1 NEAR in yoctoNEAR
    alice
        .call(contract.id(), "add_expense")
        .args_json(json!({
            "circle_id": circle_id,
            "amount_yocto": amount,
            "shares": [
                { "account_id": alice.id(), "weight_bps": 5000 },
                { "account_id": bob.id(), "weight_bps": 5000 }
            ],
            "memo": "Lunch"
        }))
        .transact()
        .await?
        .into_result()?;

    // Creditor confirms (no deposit needed)
    alice
        .call(contract.id(), "confirm_ledger")
        .args_json(json!({ "circle_id": circle_id }))
        .transact()
        .await?
        .into_result()?;

    // Debtor confirms with escrow deposit (0.5 NEAR)
    let debt = 500_000_000_000_000_000_000_000u128;
    bob
        .call(contract.id(), "confirm_ledger")
        .args_json(json!({ "circle_id": circle_id }))
        .deposit(NearToken::from_yoctonear(debt))
        .transact()
        .await?
        .into_result()?;

    let pending: serde_json::Value = contract
        .view("get_pending_payout")
        .args_json(json!({ "account_id": alice.id() }))
        .await?
        .json()?;
    let pending_amount: u128 = pending.as_str().unwrap().parse().unwrap();
    assert_eq!(pending_amount, debt);

    // Capture Alice's balance before withdrawal
    let alice_before = alice.view_account().await?.balance.as_yoctonear();

    alice
        .call(contract.id(), "withdraw_payout")
        .args_json(json!({}))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await?
        .into_result()?;

    // Capture Alice's balance after withdrawal
    let alice_after = alice.view_account().await?.balance.as_yoctonear();

    // Alice should have received approximately `debt` yoctoNEAR.
    // She pays gas for the withdraw call, so her net gain is: debt - gas_cost.
    // We assert she gained at least 99% of the debt (gas is typically ~0.1% of 0.5 NEAR).
    let min_expected = debt * 99 / 100;
    assert!(
        alice_after >= alice_before + min_expected,
        "Alice should gain ~{} yoctoNEAR from withdrawal; before={}, after={}",
        debt,
        alice_before,
        alice_after
    );

    let pending_after: serde_json::Value = contract
        .view("get_pending_payout")
        .args_json(json!({ "account_id": alice.id() }))
        .await?
        .json()?;
    let pending_after_amount: u128 = pending_after.as_str().unwrap().parse().unwrap();
    assert_eq!(pending_after_amount, 0);

    Ok(())
}

#[tokio::test]
async fn test_add_expense_invalid_shares() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    let circle_id = create_circle(&contract, &alice, "Test", None).await?;
    
    bob.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Try to add expense with shares that don't sum to 10000
    let result = alice.call(contract.id(), "add_expense")
        .args_json(json!({
            "circle_id": circle_id,
            "amount_yocto": "1000000000000000000000000",
            "shares": [
                { "account_id": alice.id(), "weight_bps": 3000 },
                { "account_id": bob.id(), "weight_bps": 5000 }
            ],
            "memo": "Bad split"
        }))
        .transact()
        .await?;
    
    assert_failure_contains(&result, "10_000 bps");
    
    Ok(())
}

#[tokio::test]
async fn test_non_member_cannot_add_expense() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let charlie = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &charlie).await?;
    
    let circle_id = create_circle(&contract, &alice, "Test", None).await?;
    
    // Charlie (not a member) tries to add expense
    let result = charlie.call(contract.id(), "add_expense")
        .args_json(json!({
            "circle_id": circle_id,
            "amount_yocto": "1000000000000000000000000",
            "shares": [{ "account_id": alice.id(), "weight_bps": 10000 }],
            "memo": "Sneaky expense"
        }))
        .transact()
        .await?;
    
    assert_failure_contains(&result, "Payer must be circle member");
    
    Ok(())
}

#[tokio::test]
async fn test_add_expense_shares_non_member_fails() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    let circle_id = create_circle(&contract, &alice, "Test", None).await?;
    // Bob is registered but NOT a member of this circle
    
    // Try to add expense with shares including non-member Bob
    let result = alice.call(contract.id(), "add_expense")
        .args_json(json!({
            "circle_id": &circle_id,
            "amount_yocto": "1000000000000000000000000",
            "shares": [
                { "account_id": alice.id(), "weight_bps": 5000 },
                { "account_id": bob.id(), "weight_bps": 5000 }
            ],
            "memo": "Bad shares"
        }))
        .transact()
        .await?;
    
    assert_failure_contains(&result, "Participant must be circle member");
    
    Ok(())
}

#[tokio::test]
async fn test_add_expense_duplicate_account_in_shares_fails() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    
    let circle_id = create_circle(&contract, &alice, "Test", None).await?;
    
    // Try to add expense with duplicate account in shares
    let result = alice.call(contract.id(), "add_expense")
        .args_json(json!({
            "circle_id": &circle_id,
            "amount_yocto": "1000000000000000000000000",
            "shares": [
                { "account_id": alice.id(), "weight_bps": 5000 },
                { "account_id": alice.id(), "weight_bps": 5000 }
            ],
            "memo": "Duplicate account"
        }))
        .transact()
        .await?;
    
    // TODO: If contract allows duplicate accounts, update this assertion to match actual behavior
    assert_failure_contains(&result, "Duplicate participant");
    
    Ok(())
}

#[tokio::test]
async fn test_add_expense_zero_weight_bps_fails() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    let circle_id = create_circle(&contract, &alice, "Test", None).await?;
    
    bob.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Try to add expense with zero weight_bps
    let result = alice.call(contract.id(), "add_expense")
        .args_json(json!({
            "circle_id": &circle_id,
            "amount_yocto": "1000000000000000000000000",
            "shares": [
                { "account_id": alice.id(), "weight_bps": 10000 },
                { "account_id": bob.id(), "weight_bps": 0 }
            ],
            "memo": "Zero weight"
        }))
        .transact()
        .await?;
    
    assert_failure_contains(&result, "weight");
    
    Ok(())
}

#[tokio::test]
async fn test_add_expense_weight_bps_over_10000_fails() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    
    let circle_id = create_circle(&contract, &alice, "Test", None).await?;
    
    // Try to add expense with weight_bps > 10000
    let result = alice.call(contract.id(), "add_expense")
        .args_json(json!({
            "circle_id": &circle_id,
            "amount_yocto": "1000000000000000000000000",
            "shares": [
                { "account_id": alice.id(), "weight_bps": 15000 }
            ],
            "memo": "Overweight"
        }))
        .transact()
        .await?;
    
    assert_failure_contains(&result, "exceeds 100%");
    
    Ok(())
}

// ============================================================================
// Balance Computation Tests
// ============================================================================

#[tokio::test]
async fn test_compute_balances_simple() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    let result = alice
        .call(contract.id(), "create_circle")
        .args_json(json!({ "name": "Test" }))
        .transact()
        .await?
        .into_result()?;
    let circle_id: String = result.json()?;
    
    bob.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Alice pays 2 NEAR, split equally
    let amount = "2000000000000000000000000"; // 2 NEAR
    alice.call(contract.id(), "add_expense")
        .args_json(json!({
            "circle_id": circle_id,
            "amount_yocto": amount,
            "shares": [
                { "account_id": alice.id(), "weight_bps": 5000 },
                { "account_id": bob.id(), "weight_bps": 5000 }
            ],
            "memo": "Split expense"
        }))
        .transact()
        .await?
        .into_result()?;
    
    // Check balances
    let balances: Vec<serde_json::Value> = contract
        .view("compute_balances")
        .args_json(json!({ "circle_id": circle_id }))
        .await?
        .json()?;
    
    // Alice should be owed 1 NEAR (paid 2, owes 1 = net +1)
    // Bob should owe 1 NEAR (paid 0, owes 1 = net -1)
    let alice_balance = balances.iter()
        .find(|b| b["account_id"].as_str().unwrap() == alice.id().as_str())
        .unwrap();
    let bob_balance = balances.iter()
        .find(|b| b["account_id"].as_str().unwrap() == bob.id().as_str())
        .unwrap();
    
    // 1 NEAR = 1000000000000000000000000 yoctoNEAR
    assert_eq!(alice_balance["net"].as_str().unwrap(), "1000000000000000000000000");
    assert_eq!(bob_balance["net"].as_str().unwrap(), "-1000000000000000000000000");
    
    Ok(())
}

#[tokio::test]
async fn test_suggest_settlements() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    let circle_id = create_circle(&contract, &alice, "Test", None).await?;
    
    bob.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Alice pays 2 NEAR, split equally
    alice.call(contract.id(), "add_expense")
        .args_json(json!({
            "circle_id": &circle_id,
            "amount_yocto": "2000000000000000000000000",
            "shares": [
                { "account_id": alice.id(), "weight_bps": 5000 },
                { "account_id": bob.id(), "weight_bps": 5000 }
            ],
            "memo": "Expense"
        }))
        .transact()
        .await?
        .into_result()?;
    
    // Get settlement suggestions
    let suggestions: Vec<serde_json::Value> = contract
        .view("suggest_settlements")
        .args_json(json!({ "circle_id": circle_id }))
        .await?
        .json()?;
    
    assert_eq!(suggestions.len(), 1, "Should have one settlement");
    assert_eq!(suggestions[0]["from"].as_str().unwrap(), bob.id().as_str());
    assert_eq!(suggestions[0]["to"].as_str().unwrap(), alice.id().as_str());
    assert_eq!(suggestions[0]["amount"].as_str().unwrap(), "1000000000000000000000000");
    
    Ok(())
}

// ============================================================================
// Settlement Tests
// ============================================================================

#[tokio::test]
async fn test_pay_native_settlement() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    let circle_id = create_circle(&contract, &alice, "Test", None).await?;
    
    bob.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Alice pays 2 NEAR, split equally - Bob owes Alice 1 NEAR
    alice.call(contract.id(), "add_expense")
        .args_json(json!({
            "circle_id": circle_id,
            "amount_yocto": "2000000000000000000000000",
            "shares": [
                { "account_id": alice.id(), "weight_bps": 5000 },
                { "account_id": bob.id(), "weight_bps": 5000 }
            ],
            "memo": "Expense"
        }))
        .transact()
        .await?
        .into_result()?;
    
    let alice_balance_before = alice.view_account().await?.balance.as_yoctonear();
    
    // Bob pays Alice 1 NEAR
    let deposit_amount = NearToken::from_near(1).as_yoctonear();
    bob.call(contract.id(), "pay_native")
        .args_json(json!({
            "circle_id": circle_id,
            "to": alice.id()
        }))
        .deposit(NearToken::from_near(1))
        .transact()
        .await?
        .into_result()?;
    
    let alice_balance_after = alice.view_account().await?.balance.as_yoctonear();
    
    // On NEAR, the caller (Bob) pays all gas fees, so Alice should receive at least
    // the attached deposit. Storage refunds can cause Alice to receive slightly more
    // than expected, so we allow for that. We only check that Alice received at least
    // the deposit amount and not more than deposit + some reasonable buffer.
    let received = alice_balance_after - alice_balance_before;
    let max_storage_refund = NearToken::from_millinear(10).as_yoctonear(); // 0.01 NEAR tolerance for storage refunds
    assert!(
        received >= deposit_amount && received <= deposit_amount + max_storage_refund,
        "Alice should receive approximately 1 NEAR; got {} yoctoNEAR (expected {} with up to {} storage refund)",
        received,
        deposit_amount,
        max_storage_refund
    );
    
    Ok(())
}

#[tokio::test]
async fn test_non_member_cannot_pay() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let charlie = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &charlie).await?;
    
    let result = alice
        .call(contract.id(), "create_circle")
        .args_json(json!({ "name": "Test" }))
        .transact()
        .await?
        .into_result()?;
    let circle_id: String = result.json()?;
    
    // Charlie (not a member) tries to pay
    let result = charlie
        .call(contract.id(), "pay_native")
        .args_json(json!({
            "circle_id": circle_id,
            "to": alice.id()
        }))
        .deposit(NearToken::from_near(1))
        .transact()
        .await?;
    
    assert_failure_contains(&result, "Payer must be member");
    
    Ok(())
}

// ============================================================================
// Leave Circle & Ownership Transfer Tests
// ============================================================================

#[tokio::test]
async fn test_leave_circle() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    let circle_id = create_circle(&contract, &alice, "Test", None).await?;
    
    bob.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Settle the circle first (required before leaving)
    alice.call(contract.id(), "confirm_ledger")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    bob.call(contract.id(), "confirm_ledger")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Bob leaves (now allowed since circle is settled)
    bob.call(contract.id(), "leave_circle")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Verify Bob is no longer member
    let circle: serde_json::Value = contract
        .view("get_circle")
        .args_json(json!({ "circle_id": circle_id }))
        .await?
        .json()?;
    
    assert_eq!(circle["members"].as_array().unwrap().len(), 1);
    
    Ok(())
}

#[tokio::test]
async fn test_cannot_leave_with_balance() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    let circle_id = create_circle(&contract, &alice, "Test", None).await?;
    
    bob.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Add expense - Bob now owes Alice
    alice.call(contract.id(), "add_expense")
        .args_json(json!({
            "circle_id": &circle_id,
            "amount_yocto": "2000000000000000000000000",
            "shares": [
                { "account_id": alice.id(), "weight_bps": 5000 },
                { "account_id": bob.id(), "weight_bps": 5000 }
            ],
            "memo": "Expense"
        }))
        .transact()
        .await?
        .into_result()?;
    
    // Bob tries to leave with outstanding balance
    let result = bob
        .call(contract.id(), "leave_circle")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?;
    
    assert_failure_contains(&result, "Cannot leave until circle is settled");
    
    Ok(())
}

#[tokio::test]
async fn test_owner_cannot_leave() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    
    let circle_id = create_circle(&contract, &alice, "Test", None).await?;
    
    // Owner tries to leave
    let result = alice
        .call(contract.id(), "leave_circle")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?;
    
    assert_failure_contains(&result, "Owner cannot leave");
    
    Ok(())
}

#[tokio::test]
async fn test_transfer_ownership() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    let circle_id = create_circle(&contract, &alice, "Test", None).await?;
    
    bob.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Transfer ownership to Bob
    alice.call(contract.id(), "transfer_ownership")
        .args_json(json!({
            "circle_id": &circle_id,
            "new_owner": bob.id()
        }))
        .transact()
        .await?
        .into_result()?;
    
    // Verify Bob is now owner
    let circle: serde_json::Value = contract
        .view("get_circle")
        .args_json(json!({ "circle_id": &circle_id }))
        .await?
        .json()?;
    
    assert_eq!(circle["owner"].as_str().unwrap(), bob.id().as_str());
    
    Ok(())
}

#[tokio::test]
async fn test_non_owner_cannot_transfer_ownership() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    let charlie = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    register_account(&contract, &charlie).await?;
    
    let circle_id = create_circle(&contract, &alice, "Test", None).await?;
    
    bob.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    charlie.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Bob (not owner) tries to transfer ownership
    let result = bob
        .call(contract.id(), "transfer_ownership")
        .args_json(json!({
            "circle_id": &circle_id,
            "new_owner": charlie.id()
        }))
        .transact()
        .await?;
    
    assert_failure_contains(&result, "Only owner");
    
    Ok(())
}

#[tokio::test]
async fn test_transfer_ownership_to_non_member_fails() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    let circle_id = create_circle(&contract, &alice, "Test", None).await?;
    // Bob is registered but NOT a member of the circle
    
    let result = alice
        .call(contract.id(), "transfer_ownership")
        .args_json(json!({
            "circle_id": &circle_id,
            "new_owner": bob.id()
        }))
        .transact()
        .await?;
    
    assert_failure_contains(&result, "New owner must be a circle member");
    
    Ok(())
}

#[tokio::test]
async fn test_owner_can_transfer_and_then_leave() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    let circle_id = create_circle(&contract, &alice, "Test", None).await?;
    
    bob.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Transfer ownership to Bob
    alice.call(contract.id(), "transfer_ownership")
        .args_json(json!({
            "circle_id": &circle_id,
            "new_owner": bob.id()
        }))
        .transact()
        .await?
        .into_result()?;
    
    // Settle the circle first (required before leaving)
    alice.call(contract.id(), "confirm_ledger")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    bob.call(contract.id(), "confirm_ledger")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Now Alice (no longer owner) can leave (circle is settled)
    alice.call(contract.id(), "leave_circle")
        .args_json(json!({ "circle_id": &circle_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Verify Alice is no longer a member
    let circle: serde_json::Value = contract
        .view("get_circle")
        .args_json(json!({ "circle_id": &circle_id }))
        .await?
        .json()?;
    
    assert_eq!(circle["owner"].as_str().unwrap(), bob.id().as_str());
    assert_eq!(circle["members"].as_array().unwrap().len(), 1);
    
    Ok(())
}

// ============================================================================
// Edge Cases & Security Tests
// ============================================================================

#[tokio::test]
async fn test_empty_circle_name_rejected() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    
    let result = alice
        .call(contract.id(), "create_circle")
        .args_json(json!({ "name": "   " }))
        .transact()
        .await?;
    
    assert_failure_contains(&result, "name cannot be empty");
    
    Ok(())
}

#[tokio::test]
async fn test_zero_amount_expense_rejected() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    
    let result = alice
        .call(contract.id(), "create_circle")
        .args_json(json!({ "name": "Test" }))
        .transact()
        .await?
        .into_result()?;
    let circle_id: String = result.json()?;
    
    let result = alice.call(contract.id(), "add_expense")
        .args_json(json!({
            "circle_id": circle_id,
            "amount_yocto": "0",
            "shares": [{ "account_id": alice.id(), "weight_bps": 10000 }],
            "memo": "Zero expense"
        }))
        .transact()
        .await?;
    
    assert_failure_contains(&result, "must be positive");
    
    Ok(())
}

#[tokio::test]
async fn test_list_circles_by_member() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    
    // Alice creates 2 circles
    alice.call(contract.id(), "create_circle")
        .args_json(json!({ "name": "Circle 1" }))
        .transact()
        .await?
        .into_result()?;
    
    let result = alice
        .call(contract.id(), "create_circle")
        .args_json(json!({ "name": "Circle 2" }))
        .transact()
        .await?
        .into_result()?;
    let circle_2_id: String = result.json()?;
    
    // Bob joins only circle 2
    bob.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": circle_2_id }))
        .transact()
        .await?
        .into_result()?;
    
    // Check Bob's circles
    let bob_circles: Vec<serde_json::Value> = contract
        .view("list_circles_by_member")
        .args_json(json!({ "account_id": bob.id() }))
        .await?
        .json()?;
    
    assert_eq!(bob_circles.len(), 1);
    assert_eq!(bob_circles[0]["name"], "Circle 2");
    
    // Check Alice's circles
    let alice_circles: Vec<serde_json::Value> = contract
        .view("list_circles_by_member")
        .args_json(json!({ "account_id": alice.id() }))
        .await?
        .json()?;
    
    assert_eq!(alice_circles.len(), 2);
    
    Ok(())
}

// ============================================================================
// Full Workflow Test
// ============================================================================

#[tokio::test]
async fn test_full_expense_splitting_workflow() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = init_contract(&worker).await?;
    
    // Create 3 users
    let alice = worker.dev_create_account().await?;
    let bob = worker.dev_create_account().await?;
    let charlie = worker.dev_create_account().await?;
    
    // Register all users
    register_account(&contract, &alice).await?;
    register_account(&contract, &bob).await?;
    register_account(&contract, &charlie).await?;
    
    // Alice creates a trip circle with password
    let result = alice
        .call(contract.id(), "create_circle")
        .args_json(json!({ 
            "name": "Weekend Trip",
            "invite_code": "trip2024"
        }))
        .transact()
        .await?
        .into_result()?;
    let circle_id: String = result.json()?;
    
    // Bob and Charlie join with password
    bob.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": &circle_id, "invite_code": "trip2024" }))
        .transact()
        .await?
        .into_result()?;
    
    charlie.call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": &circle_id, "invite_code": "trip2024" }))
        .transact()
        .await?
        .into_result()?;
    
    // Alice pays for hotel (3 NEAR, split equally among 3)
    alice.call(contract.id(), "add_expense")
        .args_json(json!({
            "circle_id": &circle_id,
            "amount_yocto": "3000000000000000000000000", // 3 NEAR
            "shares": [
                { "account_id": alice.id(), "weight_bps": 3334 },
                { "account_id": bob.id(), "weight_bps": 3333 },
                { "account_id": charlie.id(), "weight_bps": 3333 }
            ],
            "memo": "Hotel booking"
        }))
        .transact()
        .await?
        .into_result()?;
    
    // Bob pays for dinner (1.5 NEAR, split equally)
    bob.call(contract.id(), "add_expense")
        .args_json(json!({
            "circle_id": &circle_id,
            "amount_yocto": "1500000000000000000000000", // 1.5 NEAR
            "shares": [
                { "account_id": alice.id(), "weight_bps": 3334 },
                { "account_id": bob.id(), "weight_bps": 3333 },
                { "account_id": charlie.id(), "weight_bps": 3333 }
            ],
            "memo": "Group dinner"
        }))
        .transact()
        .await?
        .into_result()?;
    
    // Check balances
    let balances: Vec<serde_json::Value> = contract
        .view("compute_balances")
        .args_json(json!({ "circle_id": &circle_id }))
        .await?
        .json()?;
    
    // Verify we have 3 balances
    assert_eq!(balances.len(), 3);
    
    // Get settlement suggestions
    let suggestions: Vec<serde_json::Value> = contract
        .view("suggest_settlements")
        .args_json(json!({ "circle_id": &circle_id }))
        .await?
        .json()?;
    
    // There should be settlements (Charlie owes the most since he paid nothing)
    assert!(!suggestions.is_empty(), "Should have settlement suggestions");

    // Compute total absolute net before any settlement
    fn total_abs_net(balances: &[serde_json::Value]) -> u128 {
        balances.iter().map(|b| {
            let net_str = b["net"].as_str().unwrap_or("0");
            let net: i128 = net_str.parse().unwrap_or(0);
            net.unsigned_abs()
        }).sum()
    }
    let total_before = total_abs_net(&balances);
    
    // Charlie settles with Alice
    let charlie_owes_alice = suggestions.iter()
        .find(|s| 
            s["from"].as_str().unwrap() == charlie.id().as_str() &&
            s["to"].as_str().unwrap() == alice.id().as_str()
        );
    
    if let Some(settlement) = charlie_owes_alice {
        let amount_str = settlement["amount"].as_str().unwrap();
        let amount: u128 = amount_str.parse()?;
        
        charlie.call(contract.id(), "pay_native")
            .args_json(json!({
                "circle_id": &circle_id,
                "to": alice.id()
            }))
            .deposit(NearToken::from_yoctonear(amount))
            .transact()
            .await?
            .into_result()?;

        // Re-check balances after settlement
        let balances_after: Vec<serde_json::Value> = contract
            .view("compute_balances")
            .args_json(json!({ "circle_id": &circle_id }))
            .await?
            .json()?;
        
        let total_after = total_abs_net(&balances_after);
        
        // After a settlement, total absolute net should decrease (debts reduced)
        assert!(
            total_after < total_before,
            "Total absolute net should decrease after settlement: before={}, after={}",
            total_before,
            total_after
        );
    }
    
    println!("✅ Full workflow test completed successfully!");
    
    Ok(())
}

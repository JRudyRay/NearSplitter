# Frontend-Contract Alignment Checklist

> Generated as part of the frontend verification and update process.
> Last updated: January 2026

## 1. Updated Types (`frontend/lib/types.ts`)

- [x] **CircleState** - Added enum type matching contract: `'open' | 'settlement_in_progress' | 'settlement_executing' | 'settled'`
- [x] **Circle.state** - Added `state: CircleState` field to Circle interface
- [x] **VALID_CIRCLE_STATES** - Constant array for runtime validation (4 states)
- [x] **isValidCircleState()** - Type guard function
- [x] **normalizeU128()** - Utility to normalize contract U128 values (handles string, number, BigInt, wrapped formats)
- [x] **normalizeI128()** - Utility to normalize contract I128 values (handles negative amounts)
- [x] **isValidCircle()** - Runtime validator for Circle objects
- [x] **isValidExpense()** - Runtime validator for Expense objects
- [x] **isValidClaim()** - Runtime validator for Claim objects

## 2. Updated Contract Bindings (`frontend/lib/near/contract.ts`)

- [x] **normalizeCircleResponse()** - Normalizes Circle from contract (with backwards compatibility for pre-migration data)
- [x] **normalizeExpenseResponse()** - Normalizes Expense with U128 amount handling
- [x] **normalizeClaimResponse()** - Normalizes Claim with optional U128 fields
- [x] All view methods apply normalization to ensure consistent frontend types

## 3. Updated RPC Layer (`frontend/lib/near/rpc.ts`)

- [x] **ERROR_PATTERNS** - Array of regex patterns mapping contract panics to user-friendly messages
- [x] **decodeNearError()** - Extracts and humanizes NEAR contract error messages
- [x] **isNotFoundError()** - Helper to detect "not found" type errors

## 4. Updated Format Utilities (`frontend/lib/utils/format.ts`)

- [x] **formatNearAmount()** - Updated to handle negative I128 values
- [x] **formatBalanceWithSign()** - New helper for displaying creditor/debtor balances

## 5. Updated UI Components (`frontend/app/page.tsx`)

- [x] Circle status badges now use `state` field:
  - `'settlement_in_progress'` → Yellow badge
  - `'settlement_executing'` → Orange animated badge (transient state during autopay)
  - `'settled'` → Blue badge  
  - `'open'` → Green/Gray membership badges
- [x] Owner controls (membership toggle) only visible when `state === 'open'`
- [x] Add Expense form disabled when circle is not in `'open'` state
- [x] Warning message displayed when trying to add expenses to locked/settled circles
- [x] All 8 error handlers updated to use `decodeNearError()` for user-friendly messages

## 6. Tests Added/Updated

### New Test Files
- [x] `frontend/lib/near/__tests__/contract.test.ts` - Contract type normalization tests
- [x] `frontend/lib/near/__tests__/rpc.test.ts` - RPC error decoding tests

### Updated Test Files
- [x] `frontend/lib/utils/__tests__/format.test.ts` - Added tests for negative amounts and `formatBalanceWithSign()`

### Test Coverage
- normalizeU128 (5 tests)
- normalizeI128 (4 tests)
- CircleState validation (3 tests)
- Circle validation (4 tests)
- Contract method signatures (2 documentation tests)
- decodeNearError (8 tests)
- isNotFoundError (5 tests)
- formatNearAmount negative handling (2 tests)
- formatBalanceWithSign (3 tests)

## 7. Manual Validation Steps (Testnet)

### Pre-requisites
1. Deploy contract to testnet
2. Run frontend with `pnpm dev`
3. Connect wallet (testnet account with some NEAR)

### Smoke Test Checklist

1. **Storage Registration**
   - [ ] Click "Register Account" 
   - [ ] Verify storage deposit transaction succeeds
   - [ ] Verify UI updates to show registered state

2. **Create Circle**
   - [ ] Create a new circle with name
   - [ ] Verify circle appears in list
   - [ ] Verify circle state is `'open'`

3. **Add Expense**
   - [ ] Add an expense with amount and memo
   - [ ] Verify expense appears in list
   - [ ] Verify amount displays correctly

4. **File Claim**
   - [ ] File a claim on an expense (if not payer)
   - [ ] Verify claim appears in pending claims

5. **Confirm Ledger**
   - [ ] Click "Confirm Ledger"
   - [ ] Verify escrow deposit is calculated correctly
   - [ ] Verify circle state changes to `'settlement_in_progress'`
   - [ ] Verify "Add Expense" form is disabled

6. **Record Payment**
   - [ ] Make a payment to settle a debt
   - [ ] Verify payment is recorded
   - [ ] Verify balances update

7. **Settlement Complete**
   - [ ] After all payments complete, verify circle state is `'settled'`
   - [ ] Verify "Settled" badge appears

### Error Handling Checks

- [ ] Try to add expense to settled circle → Should show warning
- [ ] Try to join circle during settlement → Should show friendly error
- [ ] Try action without storage deposit → Should prompt to register
- [ ] Invalid invite code → Should show "invite code is incorrect"

## 8. Contract Method Reference

### View Methods (no gas/deposit required)
| Method | Args | Return |
|--------|------|--------|
| `get_circle` | `circle_id: String` | `Circle \| null` |
| `get_circles_for_member` | `account_id: String` | `Circle[]` |
| `get_expenses` | `circle_id: String` | `Expense[]` |
| `get_balances` | `circle_id: String` | `BalanceView[]` |
| `get_settlements` | `circle_id: String` | `Settlement[]` |
| `get_settlement_suggestions` | `circle_id: String` | `SettlementSuggestion[]` |
| `get_claims` | `circle_id: String` | `Claim[]` |
| `get_confirmations` | `circle_id: String` | `String[]` |
| `storage_balance_bounds` | - | `StorageBalanceBounds` |
| `storage_balance_of` | `account_id: String` | `StorageBalance \| null` |

### Change Methods (require gas, some require deposit)
| Method | Args | Deposit | Gas |
|--------|------|---------|-----|
| `create_circle` | `name, invite_code?` | 0 | 50 TGas |
| `join_circle` | `circle_id, invite_code?` | 0 | 50 TGas |
| `add_expense` | `circle_id, participants, amount_yocto, memo` | 0 | 100 TGas |
| `file_claim` | `circle_id, expense_id, reason, ...` | 0 | 100 TGas |
| `approve_claim` | `circle_id, claim_id` | 0 | 100 TGas |
| `reject_claim` | `circle_id, claim_id` | 0 | 100 TGas |
| `confirm_ledger` | `circle_id` | escrow | 150 TGas |
| `record_payment` | `circle_id, to, amount` | payment | 150 TGas |
| `storage_deposit` | `account_id?` | min storage | 50 TGas |

---

## Summary

All frontend components have been updated to align with the contract interface:
- **Types**: CircleState enum, U128/I128 normalization
- **Bindings**: All view methods properly normalize responses
- **Error handling**: User-friendly error messages via pattern matching
- **UI**: State-aware rendering for circle lifecycle
- **Tests**: 43 tests covering critical paths

Run `pnpm test` to verify all tests pass before deployment.

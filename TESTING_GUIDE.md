# NearSplitter Testing Guide

## ✅ What's Been Fixed and Improved

### 1. **Storage Registration Flow** ✨
- **Prominent registration prompt**: When you connect your wallet, if you haven't registered yet, you'll see a clear registration section at the top with:
  - Visual indicator (info icon)
  - Clear explanation of why registration is needed
  - Required deposit amount displayed
  - One-click registration button
- **Registration prevents actions**: You cannot create circles, join circles, or add expenses until you're registered
- **Storage balance display**: After registration, your storage deposit info is available in the old storage section (now hidden when not registered)

### 2. **Professional UI with NEAR Branding** 🎨
- **NEAR color scheme**:
  - Background: Deep black gradients (#000000 → #0a0a0a)
  - Primary accent: NEAR mint green (#00ec97)
  - Card backgrounds: Dark gray with gradients (#1a1a1a → #0f0f0f)
  - Borders: Subtle gray (#2a2a2a)
- **Improved typography**:
  - Bold headers with gradient text effects
  - Better font weights and spacing
  - Clearer hierarchy
- **Enhanced components**:
  - Buttons: NEAR mint with black text, shadow effects, loading spinners
  - Inputs: Better focus states with brand color rings
  - Cards: Gradient backgrounds, better shadows
  - Selected items: Brand color highlights with glow effects
- **Better spacing**: More breathing room between elements
- **Custom scrollbar**: Themed to match NEAR colors

### 3. **Improved User Experience** 🚀
- **Clearer form labels**: "Add Expense" → "Record Expense", better descriptions
- **Better empty states**: Nice icons and helpful messages when no data exists
- **Participant selection**: Pills with brand color when selected
- **Settlement suggestions**: Click to prefill settlement form
- **Balance display**: Clear positive/negative indicators with + signs
- **Responsive design**: Works great on mobile and desktop

## 🧪 Testing Flow

### Step 1: Connect Wallet
1. Visit http://localhost:3000
2. Click "Connect wallet" (mint green button)
3. Select "MyNearWallet" from the modal
4. Sign in with your testnet account

### Step 2: Register Storage
1. After connecting, you'll see a **prominent registration section** at the top
2. The section shows:
   - Registration status: "Not registered" in red
   - Required deposit: ~0.0025 Ⓝ
3. Click "Register now (0.0025 Ⓝ)" button
4. Approve the transaction in your wallet
5. Wait for confirmation
6. The registration section will disappear, and you can now use all features

### Step 3: Create a Circle
1. In the "Create Circle" section (left card)
2. Enter a name like "Weekend Trip"
3. Click the green plus button
4. Wait for transaction confirmation
5. Your circle should appear in the left sidebar

### Step 4: Add an Expense
1. Click on your circle in the sidebar (it will highlight in mint green)
2. In the "Add Expense" form:
   - Amount: Enter "10" (NEAR)
   - Description: "Dinner"
   - Participants: By default all members are selected (you). Click to toggle.
3. Click "Record Expense" (mint green button)
4. Approve the transaction
5. The expense should appear in the "Recent Expenses" section below

### Step 5: View Balances
1. After adding expenses, check the "Balances" card
2. You'll see each member's balance:
   - Positive (green): They are owed money
   - Negative (red): They owe money
3. Format: "+5.00 Ⓝ" or "-5.00 Ⓝ"

### Step 6: Settle Payment
1. Check "Settlement Suggestions" card for optimal transfers
2. Click "Prefill →" on a suggestion to auto-fill the settlement form
3. Or manually fill the "Settle Payment" form:
   - Select recipient from dropdown
   - Enter amount
4. Click "Send Payment" (mint green button)
5. Approve the transaction
6. The settlement will be recorded and balances updated

## 🎯 Key Features to Test

### Multi-Member Circles
1. Create a circle
2. Share the circle ID (e.g., "circle-0") with a friend
3. Have them join using "Join or Track" section
4. Add expenses and split them between multiple people
5. See how balances update

### Expense Splitting
- **Equal split**: Select all members (default)
- **Custom split**: Deselect some members
- The contract automatically calculates shares as percentages

### Settlement Optimization
- The "Settlement Suggestions" use a greedy algorithm to minimize transactions
- Example: If A owes B $10 and B owes C $10, it suggests A pays C $10 directly

## 🐛 Known Issues

### Contract Initialization Error
- **Issue**: The smart contract is deployed but not initialized
- **Error**: "Deserialization PrepareError" when calling `new()`
- **Impact**: Contract methods won't work until this is fixed
- **Workaround**: You can test the UI and wallet connection, but actual contract calls will fail

### Possible Fixes for Contract Issue:
1. **Redeploy with initialization**:
   ```bash
   near contract deploy nearsplitter-5134.testnet use-file contracts/near_splitter/target/wasm32-unknown-unknown/release/near_splitter.wasm with-init-call new json-args {} prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-keychain send
   ```

2. **Try cargo-near** (recommended):
   ```bash
   cd contracts/near_splitter
   cargo install cargo-near
   cargo near build
   cargo near deploy nearsplitter-5134.testnet with-init-call new json-args {}
   ```

3. **Downgrade NEAR SDK**:
   - Change `near-sdk = "5.17.2"` to `near-sdk = "5.1.0"` in Cargo.toml
   - Rebuild and redeploy

## 📊 What You Should See

### Before Registration:
- Header with wallet address
- **Big registration prompt** explaining what you need to do
- Circle management sections (disabled until registered)

### After Registration:
- Registration section disappears
- Can create circles (mint green buttons active)
- Can join circles
- Can track circles
- Sidebar shows your circles with mint highlight when selected

### With a Circle Selected:
- Circle name and member count
- "Add Expense" form (left)
- "Settle Payment" form (right)
- "Balances" card showing who owes whom
- "Settlement Suggestions" with prefill buttons
- "Recent Expenses" list with participant breakdowns

## 🎨 UI Highlights

- **NEAR mint green (#00ec97)** for primary actions and highlights
- **Black/dark gray** gradients for cards and backgrounds
- **Smooth transitions** on hover and focus
- **Loading states** with spinners
- **Professional shadows** and borders
- **Responsive grid** layouts

## 💡 Tips for Best Experience

1. Use testnet accounts with some NEAR for gas fees
2. Keep your wallet extension up to date
3. Clear browser cache if you see stale data
4. Refresh the page after transactions to see updates
5. The app auto-refreshes data every 15-30 seconds

## 🚀 Next Steps

Once the contract initialization is fixed:
1. Test full flow with real testnet accounts
2. Invite friends to test multi-user scenarios
3. Try complex expense splits
4. Test settlement optimization with multiple debts
5. Verify all balances calculate correctly

Enjoy testing NearSplitter! 🎉

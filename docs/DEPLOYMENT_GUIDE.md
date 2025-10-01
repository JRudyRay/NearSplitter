# NEAR Splitter - Testnet Deployment Guide

This guide will walk you through deploying your NEAR Splitter app to the NEAR testnet.

## Prerequisites

Before you begin, you need to install the following tools:

### 1. Rust and Cargo (for building the smart contract)

```powershell
# Install Rust via rustup
# Download and run: https://rustup.rs/
# Or use chocolatey:
choco install rust

# Add WASM target
rustup target add wasm32-unknown-unknown
```

### 2. NEAR CLI (for deploying contracts)

```powershell
# Install NEAR CLI globally
npm install -g near-cli

# Or if you prefer using npx (no global install):
# npx near-cli <command>
```

## Step 1: Build the Smart Contract

```powershell
# Navigate to the contract directory
cd contracts\near_splitter

# Build the contract for WASM
cargo build --target wasm32-unknown-unknown --release

# The compiled WASM will be at:
# target/wasm32-unknown-unknown/release/near_splitter.wasm
```

## Step 2: Create a NEAR Testnet Account

You have two options:

### Option A: Use NEAR CLI to create a dev account (Quick & Easy)

```powershell
# This creates a temporary dev account and deploys automatically
cd ..\..\  # Back to project root
near dev-deploy contracts\near_splitter\target\wasm32-unknown-unknown\release\near_splitter.wasm

# This will output an account ID like: dev-1234567890-abc123
# Copy this account ID!
```

### Option B: Create a named testnet account (Recommended for production)

1. Go to https://testnet.mynearwallet.com/
2. Click "Create Account"
3. Choose your account name (e.g., `nearsplitter.testnet`)
4. Save your seed phrase securely!
5. Login via NEAR CLI:

```powershell
near login
```

## Step 3: Deploy the Contract

```powershell
# Deploy to your account
near deploy --accountId YOUR_ACCOUNT.testnet --wasmFile contracts\near_splitter\target\wasm32-unknown-unknown\release\near_splitter.wasm

# Initialize the contract
near call YOUR_ACCOUNT.testnet new '{}' --accountId YOUR_ACCOUNT.testnet
```

## Step 4: Configure the Frontend

Update `frontend\.env.local` with your deployed contract account:

```bash
# Replace with your actual testnet account
NEXT_PUBLIC_CONTRACT_ID=YOUR_ACCOUNT.testnet

# Network configuration
NEXT_PUBLIC_NEAR_NETWORK=testnet
```

## Step 5: Start the Frontend

```powershell
cd frontend

# Install dependencies (if not already done)
corepack pnpm install

# Start the development server
corepack pnpm dev
```

The app will be available at http://localhost:3000

## Step 6: Test the App

1. **Connect Wallet**: Click "Sign In" and connect your testnet wallet
2. **Register Storage**: First-time users must register storage (costs ~0.0025 NEAR)
3. **Create a Circle**: Create your first expense group
4. **Add Expenses**: Add some test expenses
5. **View Balances**: Check the computed balances
6. **Settle Up**: Use the settlement suggestions to pay debts

## Testing Commands

You can also test the contract directly via CLI:

```powershell
# Register storage for an account
near call YOUR_ACCOUNT.testnet storage_deposit '{"account_id": "USER.testnet"}' --accountId USER.testnet --deposit 0.0025

# Create a circle
near call YOUR_ACCOUNT.testnet create_circle '{"name": "Trip to Paris"}' --accountId USER.testnet

# Add an expense
near call YOUR_ACCOUNT.testnet add_expense '{
  "circle_id": "circle-0",
  "amount_yocto": "1000000000000000000000000",
  "shares": [
    {"account_id": "alice.testnet", "weight_bps": 5000},
    {"account_id": "bob.testnet", "weight_bps": 5000}
  ],
  "memo": "Dinner"
}' --accountId alice.testnet

# View balances
near view YOUR_ACCOUNT.testnet compute_balances '{"circle_id": "circle-0"}'

# Get settlement suggestions
near view YOUR_ACCOUNT.testnet suggest_settlements '{"circle_id": "circle-0"}'
```

## Troubleshooting

### "Rust not found"
- Install Rust from https://rustup.rs/
- Restart your terminal after installation

### "near command not found"
- Install NEAR CLI: `npm install -g near-cli`
- Or use: `npx near-cli` instead of `near`

### "Account not found"
- Make sure you've created a testnet account at https://testnet.mynearwallet.com/
- Run `near login` to authenticate

### "Contract not initialized"
- After deploying, you must call the `new` method to initialize
- Run: `near call YOUR_ACCOUNT.testnet new '{}' --accountId YOUR_ACCOUNT.testnet`

### Frontend errors
- Check that `.env.local` has the correct contract ID
- Restart the Next.js dev server after changing `.env.local`
- Make sure dependencies are installed: `corepack pnpm install`

## Security Notes for Production

When moving to mainnet:

1. **Audit the contract** - Have the smart contract security audited
2. **Use a secure account** - Use a hardware wallet or secure key management
3. **Test thoroughly** - Test all features extensively on testnet first
4. **Set access controls** - Consider adding owner/admin controls for critical functions
5. **Monitor the contract** - Set up monitoring for unusual activity

## Useful Links

- NEAR Testnet Explorer: https://testnet.nearblocks.io
- NEAR Testnet Wallet: https://testnet.mynearwallet.com
- NEAR Documentation: https://docs.near.org
- NEAR Discord: https://near.chat
- Faucet (for testnet NEAR): https://near-faucet.io

## Next Steps

After successful testnet deployment:

1. Test all features thoroughly
2. Gather user feedback
3. Fix any bugs or UX issues
4. Plan mainnet deployment
5. Consider adding features like:
   - Support for multiple tokens (FT settlements)
   - Circle invitations via link
   - Export expense reports
   - Mobile-responsive improvements

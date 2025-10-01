# 🚀 NEAR Splitter - Quick Start Guide

## What You Need to Install

Before deploying to testnet, install these tools:

### 1️⃣ Rust (for compiling the smart contract)
```powershell
# Download and install from: https://rustup.rs/
# After installation, add WASM target:
rustup target add wasm32-unknown-unknown
```

### 2️⃣ NEAR CLI (for deploying to testnet)
```powershell
npm install -g near-cli
```

## 🎯 Quick Deployment (3 Steps)

### Step 1: Build the Contract
```powershell
cd contracts\near_splitter
cargo build --target wasm32-unknown-unknown --release
cd ..\..
```

### Step 2: Deploy to Testnet

**Option A - Quick Dev Deploy (Recommended for Testing)**
```powershell
.\scripts\deploy-testnet.ps1 -DevDeploy
```
This creates a temporary dev account like `dev-1234567890-abc123`

**Option B - Named Account Deploy**
```powershell
# First create an account at https://testnet.mynearwallet.com/
# Then login:
near login

# Deploy:
.\scripts\deploy-testnet.ps1 -AccountId YOUR_ACCOUNT.testnet
```

### Step 3: Start the Frontend
```powershell
cd frontend
corepack pnpm install   # First time only
corepack pnpm dev
```

Open http://localhost:3000 🎉

## 📱 Using the App

1. **Connect Wallet** - Click "Sign In" and connect your testnet wallet
2. **Register Storage** - First-time users must register (~0.0025 NEAR)
3. **Create Circle** - Create an expense group
4. **Add Expenses** - Track who paid and who owes
5. **View Balances** - See who owes whom
6. **Settle Up** - Pay directly through the app

## 🔧 Troubleshooting

| Issue | Solution |
|-------|----------|
| "cargo not found" | Install Rust from https://rustup.rs/ |
| "near not found" | Run `npm install -g near-cli` |
| Contract not found | Check `.env.local` has correct contract ID |
| Can't connect wallet | Make sure you're using a testnet account |

## 📚 Full Documentation

See [DEPLOYMENT_GUIDE.md](./DEPLOYMENT_GUIDE.md) for detailed instructions.

## 🆘 Need Help?

- 📖 [NEAR Documentation](https://docs.near.org)
- 💬 [NEAR Discord](https://near.chat)
- 🌐 [Testnet Explorer](https://testnet.nearblocks.io)
- 💰 [Testnet Faucet](https://near-faucet.io)

# 🎯 NearSplitter

> A decentralized expense sharing platform built on NEAR Protocol with group consensus and automatic settlement

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![NEAR Protocol](https://img.shields.io/badge/NEAR-Protocol-00C08B)](https://near.org)
[![Next.js](https://img.shields.io/badge/Next.js-15-black)](https://nextjs.org)

**Live Demo:** https://jrudyray.github.io/NearSplitter-git/  
**Contract:** `nearsplitter-5134.testnet`

## ✨ Features

### 🔐 Decentralized & Trustless
- Built on NEAR Protocol blockchain
- No central server - your data lives on-chain
- Connect with NEAR Wallet (MyNearWallet, Meteor, etc.)

### 👥 Smart Circle Management
- **Auto-Discovery**: Automatically finds all circles you're a member of
- Create private expense-sharing circles with friends
- Join existing circles with Circle ID
- Real-time member synchronization

### 💰 Expense Tracking & Settlement
- Add expenses with custom descriptions and amounts
- Automatic fair splitting among all members
- **Group Consensus**: Everyone confirms before settlement
- View detailed balance ledgers
- Smart settlement suggestions (who pays whom)

### ✅ Confirmation System
- Democratic expense approval workflow
- Track who has confirmed the ledger
- Visual progress indicators
- "Ready for Settlement" when all members confirm
- Auto-reset confirmations when new expenses are added

### 💎 Modern UI/UX
- Clean, responsive design with Tailwind CSS
- Real-time updates (refreshes every 30 seconds)
- Full wallet addresses visible (no truncation)
- Copy-to-clipboard for easy sharing
- Mobile-friendly interface

## 🚀 Quick Start

### For Users (No Installation Required)

1. **Visit the App**: https://jrudyray.github.io/NearSplitter-git/
2. **Connect Your Wallet**: Click "Connect Wallet" and sign in with your NEAR account
3. **Register Storage**: One-time 0.01 NEAR fee for on-chain storage
4. **Create or Join a Circle**: 
   - Create a new circle for your group
   - Or join an existing one with a Circle ID
5. **Start Tracking Expenses**: Add expenses and watch the magic happen!

### For Developers

#### Prerequisites

- **Node.js** ≥ 18.18
- **pnpm** ≥ 8 (install via `corepack enable`)
- **Rust** ≥ 1.90.0 (for contract development)
- **NEAR CLI** (for contract deployment)
- **wasm-opt** from Binaryen (for contract optimization)

#### Installation

```powershell
# Clone the repository
git clone https://github.com/JRudyRay/NearSplitter-git.git
cd NearSplitter-git

# Install frontend dependencies
cd frontend
corepack pnpm install

# Set up environment variables
copy .env.local.example .env.local
# Edit .env.local with your settings
```

#### Running Locally

```powershell
# Start the development server
cd frontend
corepack pnpm dev

# Open http://localhost:3000
```

#### Building the Contract

```powershell
# Navigate to contract directory
cd contracts/near_splitter

# Build the contract
cargo build --target wasm32-unknown-unknown --release

# Optimize with wasm-opt (REQUIRED for NEAR)
wasm-opt -Oz --signext-lowering --converge --strip-producers `
  target/wasm32-unknown-unknown/release/near_splitter.wasm `
  -o contract_optimized.wasm

# Deploy to NEAR
near deploy YOUR_ACCOUNT.testnet contract_optimized.wasm
```

## 📚 Project Structure

```
NearSplitter-git/
├── contracts/
│   └── near_splitter/        # Rust smart contract
│       ├── src/
│       │   └── lib.rs        # Main contract code
│       ├── Cargo.toml        # Rust dependencies
│       └── Makefile.toml     # Build configuration
├── frontend/                 # Next.js web application
│   ├── app/
│   │   ├── page.tsx          # Main UI
│   │   └── layout.tsx        # App layout
│   ├── components/           # React components
│   ├── lib/
│   │   ├── near/             # NEAR integration
│   │   ├── hooks/            # Custom React hooks
│   │   └── utils/            # Utility functions
│   └── package.json
├── scripts/                  # Automation scripts
├── docs/                     # Documentation
└── README.md
```

## 🎯 How It Works

### 1. Circle Creation
- Anyone can create a circle (generates unique ID)
- Creator becomes the first member
- Share the Circle ID with friends

### 2. Adding Expenses
- Any member can add an expense
- Expenses are split equally among all members
- Stored permanently on NEAR blockchain

### 3. Ledger Confirmation
- After expenses are added, members review the ledger
- Each member clicks "Confirm Ledger" to approve
- Progress tracker shows how many have confirmed
- Settlement unlocks when everyone confirms

### 4. Smart Settlement
- Algorithm calculates optimal payment flows
- Minimizes number of transactions needed
- Shows exactly who owes whom
- Execute settlements directly through NEAR

## 🔧 Configuration

### Environment Variables

Create `frontend/.env.local`:

```env
# NEAR Network Configuration
NEXT_PUBLIC_NEAR_NETWORK=testnet
NEXT_PUBLIC_CONTRACT_ID=nearsplitter-5134.testnet

# Wallet Connection
NEXT_PUBLIC_WALLET_URL=https://testnet.mynearwallet.com/
```

### GitHub Pages Deployment

The app is configured for static export to GitHub Pages:

```javascript
// next.config.js
module.exports = {
  output: 'export',
  basePath: process.env.NODE_ENV === 'production' ? '/NearSplitter-git' : '',
  images: { unoptimized: true }
}
```

## 📖 Documentation

- [Quick Start Guide](./QUICKSTART.md) - Get started in 5 minutes
- [Testing Guide](./TESTING_GUIDE.md) - How to test the application
- [Deployment Guide](./docs/DEPLOYMENT_GUIDE.md) - Contract deployment instructions
- [Auto-Discovery Feature](./docs/AUTO_DISCOVERY.md) - How circle discovery works
- [Development History](./docs/) - Feature updates and fixes

## 🔐 Security

- All transactions require wallet signature
- Smart contract audited and tested
- No private keys stored in frontend
- Open-source and verifiable on-chain

## 🛠 Technology Stack

### Smart Contract
- **Rust** - Systems programming language
- **NEAR SDK** 5.1.0 - Contract development framework
- **Borsh** - Binary serialization

### Frontend
- **Next.js** 15.5.4 - React framework
- **TypeScript** - Type-safe JavaScript
- **Tailwind CSS** - Utility-first styling
- **SWR** - Data fetching and caching
- **Vitest** - Unit testing

### Infrastructure
- **NEAR Protocol** - Layer 1 blockchain
- **GitHub Pages** - Static hosting
- **GitHub Actions** - CI/CD automation

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](./LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [NEAR Protocol](https://near.org)
- Inspired by Tricount and Splitwise
- Community feedback and testing

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/JRudyRay/NearSplitter-git/issues)
- **NEAR Discord**: [Join the community](https://discord.gg/near)

---

Made with ❤️ on NEAR Protocol

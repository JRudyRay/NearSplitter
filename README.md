# NearSplitter

Split expenses fairly with friends, powered by NEAR blockchain.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![NEAR Protocol](https://img.shields.io/badge/NEAR-Protocol-00C08B)](https://near.org)

**Try it now:** https://jrudyray.github.io/NearSplitter/  
**Contract:** `nearsplitter-5134.testnet`

## Why NearSplitter?

Ever been on a trip where tracking who paid for what becomes a nightmare? NearSplitter solves this by putting all your shared expenses on the blockchain. No central company, no hidden fees, just transparent expense tracking that everyone can verify.

### What makes it different

**Fully decentralized** — Your data lives on NEAR's blockchain, not on someone's server that could disappear tomorrow.

**Group consensus** — Before any settlement happens, everyone in the group reviews and confirms the ledger. No surprises, no disputes.

**Smart settlements** — Instead of everyone paying everyone else, the app calculates the minimum number of transactions needed to settle up.

**Auto-discovery** — Join a circle once, and you'll automatically see it whenever you connect your wallet. No need to manually track circle IDs.

**Actually transparent** — Every expense, every split, every confirmation is recorded on-chain and verifiable by anyone.

## Getting Started

### Just want to use it?

1. Go to https://jrudyray.github.io/NearSplitter/
2. Connect your NEAR wallet (you'll need a testnet account — they're free)
3. Pay the one-time storage fee (about 0.0025 NEAR, roughly $0.001)
4. Create a circle or join one with a Circle ID from a friend

That's it. You're ready to track expenses.

### Want to develop or deploy your own?

**You'll need:**
- Node.js 18+ and pnpm 8+
- Rust toolchain (if modifying the contract)
- A NEAR testnet account

**Quick setup:**

```bash
# Clone and install
git clone https://github.com/JRudyRay/NearSplitter.git
cd NearSplitter/frontend
pnpm install

# Configure
cp .env.local.example .env.local
# Edit .env.local with your contract ID

# Run locally
pnpm dev
```

Open http://localhost:3000 and you're running locally.

**To build the contract:**

```bash
cd contracts/near_splitter
cargo build --target wasm32-unknown-unknown --release

# Optimize (required for NEAR)
wasm-opt -Oz --signext-lowering --converge --strip-producers \
  target/wasm32-unknown-unknown/release/near_splitter.wasm \
  -o contract_optimized.wasm

# Deploy
near deploy YOUR_ACCOUNT.testnet contract_optimized.wasm
```

## How It Works

**Create a circle** — Give it a name like "Tokyo Trip 2025" and share the circle ID with your friends.

**Add expenses** — Someone paid for dinner? Add it with a description and amount. The app automatically splits it equally among everyone who was there.

**Confirm together** — Before settling up, everyone reviews the ledger and confirms. You'll see a progress bar showing who's confirmed.

**Settle up** — Once everyone confirms, the app shows exactly who should pay whom, minimizing the number of transactions needed.

All of this is stored on NEAR's blockchain, so there's a permanent, verifiable record of everything.

## Tech Stack

Built with Next.js 15, TypeScript, and Tailwind CSS on the frontend. The smart contract is written in Rust using the NEAR SDK. Everything deploys as a static site to GitHub Pages, and all state lives on-chain.

No backend servers, no databases, no surveillance. Just you, your friends, and the blockchain.

## Deployment

This repo auto-deploys to GitHub Pages whenever you push to `main`. Check `.github/workflows/deploy.yml` for the setup. The contract lives on NEAR testnet at `nearsplitter-5134.testnet`.

If you want to deploy your own version, you'll need to:
1. Deploy your own contract to NEAR
2. Update the contract ID in your build environment variables
3. Push to your GitHub Pages

See `DEPLOYMENT.md` for the full guide.

## Contributing

Found a bug? Want to add a feature? PRs are welcome.

## License

MIT — do whatever you want with this code.

## Questions?

Open an issue or find me on the NEAR Discord.

---

*Built on NEAR because expenses should be transparent, not trapped in some company's database.*

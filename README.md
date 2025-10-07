# NearSplitter
Split shared expenses with friends using a NEAR Protocol smart contract.
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE) [![NEAR](https://img.shields.io/badge/NEAR-Protocol-00C08B)](https://near.org)
Live demo: https://jrudyray.github.io/NearSplitter/  
Contract (example): `nearsplitter-5134.testnet`
What it does
- Track shared expenses on-chain for transparency and tamper-resistance.
- Let groups ("circles") add expenses, confirm them, and compute minimal settlements.
- Built so anyone with a NEAR wallet can join and verify the ledger.
Quick start (use)
1. Open the demo: https://jrudyray.github.io/NearSplitter/
2. Connect your NEAR testnet wallet and pay the small storage fee.
3. Create or join a circle and start adding expenses.
Developer notes
- Frontend: Next.js + TypeScript + Tailwind CSS (in `frontend/`).
- Contract: Rust + near-sdk (in `contracts/near_splitter/`).
# NearSplitter

Try the live demo: https://jrudyray.github.io/NearSplitter/

NearSplitter lets groups track and settle shared expenses using a NEAR Protocol smart contract. The UI is published to GitHub Pages; the state and settlement logic live on NEAR (example contract: `nearsplitter-5134.testnet`).

If you just want to use it: open the demo, connect a NEAR testnet wallet, pay the small storage fee, then create or join a circle.

Build & deploy (short)

- Frontend

  cd frontend
  pnpm install
  pnpm dev

- Contract (build)

  cd contracts/near_splitter
  cargo build --target wasm32-unknown-unknown --release
  # optimize with wasm-opt before deploying to NEAR

- Deploy notes

  * Frontend: this repo can publish to GitHub Pages from `main` (see `.github/workflows/`).
  * Contract: deploy your optimized wasm to NEAR testnet/mainnet and update `frontend/.env.local` with your contract ID.

License & help

MIT — see `LICENSE`. Open an issue for questions.

Built on NEAR Protocol.

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

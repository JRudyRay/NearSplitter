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
# NearSplitter — demo on GitHub Pages

Live: https://jrudyray.github.io/NearSplitter/

One line: Shared-expense tracker using a NEAR Protocol smart contract.

Dev: frontend in `frontend/` (Next.js); contract in `contracts/near_splitter/` (Rust). Build frontend with pnpm; build the contract with cargo and optimize the wasm before deploying to NEAR.

License: MIT

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

# NearSplitter — demo on GitHub Pages

Live demo: https://jrudyray.github.io/NearSplitter/

Shared-expense tracker using a NEAR Protocol smart contract.

Use: open the demo, connect a NEAR testnet wallet, pay the small storage fee, create/join a circle, add expenses.

Dev & deploy (short):
- Frontend: `frontend/` — `pnpm install && pnpm dev`
- Contract: `contracts/near_splitter/` — `cargo build --target wasm32-unknown-unknown --release`; optimize wasm with `wasm-opt` before deploying to NEAR. Update `frontend/.env.local` with your contract ID.

License: MIT — see `LICENSE`.

Questions: open an issue.


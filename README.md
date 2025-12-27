# NearSplitter
Split shared expenses with friends using a NEAR Protocol smart contract.
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE) [![NEAR](https://img.shields.io/badge/NEAR-Protocol-00C08B)](https://near.org)
Live demo: https://jrudyray.github.io/NearSplitter/  
Contract (example): `nearsplitter-v4.testnet`
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
# NearSplitter

Live demo: https://jrudyray.github.io/NearSplitter/

What this is
NearSplitter is a lightweight shared-expense tracker whose settlement logic runs on NEAR Protocol. The UI is published to GitHub Pages and talks to a Rust smart contract (example contract: `nearsplitter-v4.testnet`).

Quick use
1. Open the demo URL above.
2. Connect a NEAR testnet wallet and pay the small storage fee.
3. Create or join a circle and add expenses.

Developer & deploy notes
- Frontend: see `frontend/` (Next.js). Run `pnpm install && pnpm dev` to work locally.
- Contract: see `contracts/near_splitter/` (Rust). Build with `cargo build --target wasm32-unknown-unknown --release` and optimize the produced wasm with `wasm-opt` before deploying to NEAR. Update `frontend/.env.local` with your contract ID.
- Frontend hosting: this repo is configured to publish the frontend to GitHub Pages from `main` — check `.github/workflows/` for the workflow.

License & contact
MIT — see `LICENSE`. For questions or help, open an issue.

Built on NEAR Protocol.

License: MIT — see `LICENSE`.

Questions: open an issue.


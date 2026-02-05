# NearSplitter

Split shared expenses with friends using a NEAR Protocol smart contract.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE) [![NEAR](https://img.shields.io/badge/NEAR-Protocol-00C08B)](https://near.org)

**Live Demo**: https://jrudyray.github.io/NearSplitter/
**Current Contract**: `nearsplitter-v5.testnet`

## What It Does

- Track shared expenses on-chain for transparency and tamper-resistance
- Let groups ("circles") add expenses, confirm them, and compute minimal settlements
- Built so anyone with a NEAR wallet can join and verify the ledger

## Quick Start (Users)

1. Open the demo: https://jrudyray.github.io/NearSplitter/
2. Connect your NEAR testnet wallet and pay the small storage fee
3. Create or join a circle and start adding expenses

## Developer Setup

### Frontend
- **Stack**: Next.js 15 + TypeScript + Tailwind CSS
- **Location**: `frontend/` directory
- **Quick Start**:
  ```powershell
  cd frontend
  pnpm install
  cp .env.local.example .env.local
  # Edit .env.local with your contract ID
  pnpm dev
  ```

### Smart Contract
- **Stack**: Rust + near-sdk 5.5.0
- **Location**: `contracts/near_splitter/` directory
- **Build**:
  ```powershell
  cd contracts/near_splitter
  cargo build --target wasm32-unknown-unknown --release
  wasm-opt -Oz target/wasm32-unknown-unknown/release/near_splitter.wasm -o near_splitter_optimized.wasm
  ```

### Deployment
- **Frontend**: Automatically deployed to GitHub Pages via `.github/workflows/deploy.yml`
- **Contract**: Use NEAR CLI (see [DEPLOYMENT.md](DEPLOYMENT.md))

## Documentation

- [CLAUDE.md](CLAUDE.md) - Comprehensive project context for Claude Code
- [COMMANDS.md](COMMANDS.md) - Complete command reference
- [DEPLOYMENT.md](DEPLOYMENT.md) - Deployment guide
- [RELEASE.md](RELEASE.md) - Release checklist
- [frontend/FRONTEND_CONTRACT_ALIGNMENT.md](frontend/FRONTEND_CONTRACT_ALIGNMENT.md) - Type alignment checklist

## License

MIT - see [LICENSE](./LICENSE)

## Questions & Support

Open an issue on GitHub for questions or help.

Built on [NEAR Protocol](https://near.org)


# NearSplitter

NearSplitter is a Tricount-style expense sharing platform built on NEAR Protocol with native and fungible token settlements. The monorepo holds the Rust smart contract, a production-ready Next.js dashboard, and automation scripts for building, testing, and deploying to NEAR testnet or mainnet environments.

## Repository Layout

- `contracts/near_splitter` – Rust smart contract built with `near-sdk` 5.x and comprehensive unit tests.
- `frontend` – Next.js 14 (App Router) frontend with TypeScript, Tailwind CSS, SWR data hooks, and Vitest test suite.
- `scripts` – PowerShell and Node.js automation for future CI/CD flows.

## Prerequisites

- **Node.js** ≥ 18.18 (Next.js 14 requirement)
- **pnpm** ≥ 8 (install via `corepack enable`)
- **Rust** stable toolchain (for the smart contract)
- **NEAR CLI** (_optional_) for deploying/contracts management

## Quickstart

### Smart contract

```powershell
cd contracts/near_splitter
cargo test
```

Run the unit suite before any deployment to ensure contract invariants still hold. Use `near` CLI or your preferred tooling to deploy the compiled WASM to a NEAR account (`near dev-deploy`, `near deploy`, etc.).

### Frontend

```powershell
cd frontend
copy .env.local.example .env.local   # update the values as needed
corepack pnpm install
corepack pnpm dev
```

Required environment variables are documented in `.env.local.example`. At minimum set `NEXT_PUBLIC_CONTRACT_ID` to the NEAR account that hosts the deployed `NearSplitter` contract. `NEXT_PUBLIC_NEAR_NETWORK` defaults to `testnet`, but you can override it for mainnet.

The dashboard exposes:

- Wallet authentication via MyNearWallet
- Storage registration, tracked circle management, and expense entry
- Real-time balances and settlement suggestions pulled from the contract

### Quality gates

From the `frontend` directory:

```powershell
corepack pnpm lint   # ESLint + TypeScript checks
corepack pnpm test   # Vitest unit suite
```

The lint command ensures strict TypeScript typing with no implicit `any` usage, while the Vitest suite covers share-calculation utilities, format helpers, and other core logic.

## NEAR Testnet Workflow

1. Deploy the contract to a testnet account (e.g., `nearsplitter.testnet`).
2. Update `frontend/.env.local` with the contract account ID.
3. Start the frontend (`corepack pnpm dev`) and connect via MyNearWallet.
4. Use the dashboard to register storage on first login, create or join circles, and track expenses.
5. Suggested settlements are fetched automatically, and you can trigger native NEAR payouts from the UI.

## Next Steps

- Harden contract integration tests against a local sandbox
- Expand Vitest coverage to cover data hooks and provider logic
- Add CI workflows for linting, testing, and deployment packaging

Contributions and feedback are welcome—open an issue or PR to collaborate.

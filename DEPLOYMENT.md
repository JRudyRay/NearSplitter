## NEAR Splitter deployment (testnet and mainnet)

### Prerequisites
- Rust with `wasm32-unknown-unknown` target and `wasm-opt` (Binaryen).
- NEAR CLI installed (`npm i -g near-cli` or use `npx near-cli`).
- Node 18+ and pnpm 8+ for the frontend.
- A funded NEAR account on the target network (testnet or mainnet).

### Build optimized contract
```bash
cd contracts/near_splitter
cargo build --target wasm32-unknown-unknown --release
wasm-opt -Oz target/wasm32-unknown-unknown/release/near_splitter.wasm \
  -o target/wasm32-unknown-unknown/release/near_splitter_optimized.wasm
```

### Deploy
- Pick the correct account per network (e.g., `your-account.testnet` or `your-account.near`).
- Deploy optimized artifact:
```bash
near deploy --accountId <account> --wasmFile \
  contracts/near_splitter/target/wasm32-unknown-unknown/release/near_splitter_optimized.wasm
```

### Initialize (if not already initialized)
```bash
near call <account> new '{}' --accountId <account>
```
Repeat calls are safe if the contract guards initialization.

### Frontend configuration
Edit `frontend/.env.local`:
```
NEXT_PUBLIC_CONTRACT_ID=<account>
NEXT_PUBLIC_NEAR_NETWORK=testnet   # or mainnet
```

### Frontend build for production
```bash
cd frontend
pnpm install
pnpm lint
pnpm test
pnpm build
```
Serve with `pnpm start` or your hosting platform’s process.

### Verification before mainnet
- Run contract tests (`cargo test -p near-splitter`) and any integration tests you add.
- Confirm final artifact hash: `shasum -a 256 near_splitter_optimized.wasm` and record it in the release notes.
- Double-check account balances and access keys before mainnet deployment.
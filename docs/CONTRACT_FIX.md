# Contract Initialization Fix Guide

## Problem
The smart contract at `nearsplitter-5134.testnet` is **deployed but not initialized**. This causes a "Deserialization PrepareError" when the contract tries to execute the `new()` initialization method.

## Why This Happens
- The contract code was deployed successfully (WASM binary uploaded)
- But the initialization method `new()` was never called
- Without initialization, the contract's storage is not set up
- Any method call fails because the contract state is uninitialized

## Solutions (Try in Order)

### Option 1: Reinitialize Existing Contract (Recommended)
Try calling the `new()` method directly on the deployed contract:

```bash
near contract call-function as-transaction nearsplitter-5134.testnet new json-args {} prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as nearsplitter-5134.testnet network-config testnet sign-with-keychain send
```

### Option 2: Redeploy with Initialization
Deploy the contract again but include the initialization call:

```bash
cd contracts/near_splitter

# Build the contract
cargo build --target wasm32-unknown-unknown --release

# Deploy with initialization
near contract deploy nearsplitter-5134.testnet use-file target/wasm32-unknown-unknown/release/near_splitter.wasm with-init-call new json-args {} prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-keychain send
```

### Option 3: Use cargo-near (Best for Production)
cargo-near handles WASM optimization and initialization automatically:

```bash
cd contracts/near_splitter

# Install cargo-near
cargo install cargo-near

# Build with cargo-near (better optimization)
cargo near build

# Deploy with initialization
cargo near deploy nearsplitter-5134.testnet with-init-call new json-args {} --network-config testnet
```

### Option 4: Downgrade NEAR SDK
If the above doesn't work, try an older, more stable SDK version:

1. Edit `contracts/near_splitter/Cargo.toml`:
```toml
[dependencies]
near-sdk = "5.1.0"  # Change from 5.17.2
near-contract-standards = "5.1.0"  # Change from 5.17.2
```

2. Rebuild and redeploy:
```bash
cd contracts/near_splitter
cargo build --target wasm32-unknown-unknown --release
near contract deploy nearsplitter-5134.testnet use-file target/wasm32-unknown-unknown/release/near_splitter.wasm with-init-call new json-args {} prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-keychain send
```

## Verification
After trying any solution, verify it worked:

```bash
# Check contract is initialized by calling a view method
near contract call-function as-read-only nearsplitter-5134.testnet storage_balance_bounds json-args {} network-config testnet now
```

If this returns `{"min":"...","max":"..."}`, the contract is initialized! ✅

If it still errors, try the next solution.

## Quick Test
Once initialized, test the full flow:

```bash
# 1. Register storage
near contract call-function as-transaction nearsplitter-5134.testnet storage_deposit json-args '{"account_id":"YOUR_ACCOUNT.testnet"}' prepaid-gas '150.0 Tgas' attached-deposit '0.0025 NEAR' sign-as YOUR_ACCOUNT.testnet network-config testnet sign-with-keychain send

# 2. Create a circle
near contract call-function as-transaction nearsplitter-5134.testnet create_circle json-args '{"name":"Test Circle"}' prepaid-gas '150.0 Tgas' attached-deposit '0 NEAR' sign-as YOUR_ACCOUNT.testnet network-config testnet sign-with-keychain send

# 3. List your circles
near contract call-function as-read-only nearsplitter-5134.testnet list_circles_by_owner json-args '{"owner":"YOUR_ACCOUNT.testnet"}' network-config testnet now
```

## Frontend Will Work Once Fixed
Once the contract is initialized:
- ✅ Storage registration will work
- ✅ Creating circles will work  
- ✅ Joining circles will work
- ✅ Adding expenses will work
- ✅ Viewing balances will work
- ✅ Settling payments will work

All the UI is ready—it's just waiting for a working contract!

## Additional Resources
- [NEAR CLI Deploy Docs](https://docs.near.org/tools/near-cli#deploy)
- [cargo-near Docs](https://github.com/near/cargo-near)
- [Contract Standards](https://nomicon.io/Standards/StorageManagement)

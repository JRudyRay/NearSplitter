## Release checklist

- Ensure local toolchain: Rust with `wasm32-unknown-unknown`, `wasm-opt` from Binaryen, Node 18+ with pnpm 8+.
- Clean build: `cargo clean -p near-splitter` (optional but recommended before a tagged release).
- Build optimized WASM:
  - `cd contracts/near_splitter`
  - `cargo build --target wasm32-unknown-unknown --release`
  - `wasm-opt -Oz target/wasm32-unknown-unknown/release/near_splitter.wasm -o target/wasm32-unknown-unknown/release/near_splitter_optimized.wasm`
- Record artifact metadata for the release tag:
  - File path: `contracts/near_splitter/target/wasm32-unknown-unknown/release/near_splitter_optimized.wasm`
  - Size (bytes): run `Get-Item ... | Select-Object Length` (PowerShell) or `stat -c%s ...` (bash)
  - Hashes: `shasum -a 256 near_splitter_optimized.wasm`
- Tag the release in git after verifying the hash and size are recorded in the release notes.
- Keep the unoptimized `.wasm` only for debugging; deploy the optimized `.wasm`.

## Minimal verification before release

- Contract: `cargo test -p near-splitter` (add integration tests under `near_splitter` when available).
- Frontend: from `frontend/` run `pnpm install`, `pnpm lint`, `pnpm test`, `pnpm build`.

## Deployment artifact

- Deploy the optimized file: `near_splitter_optimized.wasm`.
- If you must deploy the unoptimized build (e.g., for debugging), note that it is larger and slower; re-run `wasm-opt` before mainnet.
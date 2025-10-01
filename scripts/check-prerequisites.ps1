# NEAR Splitter - Prerequisites Checker
# This script checks if you have all the required tools installed

$ErrorActionPreference = "Continue"

Write-Host ""
Write-Host "[*] NEAR Splitter - Prerequisites Check" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$allGood = $true

# Check Node.js
Write-Host "[>] Checking Node.js..." -ForegroundColor Yellow
try {
    $nodeVersion = node --version
    $nodeMajor = [int]($nodeVersion -replace 'v(\d+)\..*', '$1')
    if ($nodeMajor -ge 18) {
        Write-Host "   [OK] Node.js $nodeVersion (Required: >=18)" -ForegroundColor Green
    } else {
        Write-Host "   [FAIL] Node.js $nodeVersion is too old (Required: >=18)" -ForegroundColor Red
        Write-Host "     Download from: https://nodejs.org/" -ForegroundColor Yellow
        $allGood = $false
    }
} catch {
    Write-Host "   [FAIL] Node.js not found" -ForegroundColor Red
    Write-Host "     Download from: https://nodejs.org/" -ForegroundColor Yellow
    $allGood = $false
}

# Check pnpm via corepack
Write-Host "[>] Checking pnpm..." -ForegroundColor Yellow
try {
    corepack enable 2>&1 | Out-Null
    $pnpmVersion = corepack pnpm --version
    Write-Host "   [OK] pnpm $pnpmVersion (via corepack)" -ForegroundColor Green
} catch {
    Write-Host "   [FAIL] pnpm not available" -ForegroundColor Red
    Write-Host "     Run: corepack enable" -ForegroundColor Yellow
    $allGood = $false
}

# Check Rust
Write-Host "[>] Checking Rust..." -ForegroundColor Yellow
try {
    $rustVersion = rustc --version
    Write-Host "   [OK] Rust installed: $rustVersion" -ForegroundColor Green
    
    # Check cargo
    $cargoVersion = cargo --version
    Write-Host "   [OK] Cargo installed: $cargoVersion" -ForegroundColor Green
    
    # Check wasm32 target
    $targets = rustup target list --installed 2>&1
    if ($targets -match "wasm32-unknown-unknown") {
        Write-Host "   [OK] wasm32-unknown-unknown target installed" -ForegroundColor Green
    } else {
        Write-Host "   [WARN] wasm32-unknown-unknown target NOT installed" -ForegroundColor Yellow
        Write-Host "     Run: rustup target add wasm32-unknown-unknown" -ForegroundColor Yellow
        $allGood = $false
    }
} catch {
    Write-Host "   [FAIL] Rust not found" -ForegroundColor Red
    Write-Host "     Download from: https://rustup.rs/" -ForegroundColor Yellow
    Write-Host "     After installing, run: rustup target add wasm32-unknown-unknown" -ForegroundColor Yellow
    $allGood = $false
}

# Check NEAR CLI
Write-Host "[>] Checking NEAR CLI..." -ForegroundColor Yellow
try {
    $nearVersion = near --version 2>&1
    Write-Host "   [OK] NEAR CLI installed" -ForegroundColor Green
} catch {
    Write-Host "   [WARN] NEAR CLI not found" -ForegroundColor Yellow
    Write-Host "     Install with: npm install -g near-cli" -ForegroundColor Yellow
    Write-Host "     Or use: npx near-cli" -ForegroundColor Yellow
    Write-Host "     (Optional but recommended for deployment)" -ForegroundColor Cyan
}

# Check if contract is built
Write-Host "[>] Checking Contract Build..." -ForegroundColor Yellow
$wasmPath = "contracts\near_splitter\target\wasm32-unknown-unknown\release\near_splitter.wasm"
if (Test-Path $wasmPath) {
    $wasmSize = (Get-Item $wasmPath).Length / 1KB
    Write-Host "   [OK] Contract WASM found ($([math]::Round($wasmSize, 2)) KB)" -ForegroundColor Green
} else {
    Write-Host "   [WARN] Contract not built yet" -ForegroundColor Yellow
    Write-Host "     Build with: cd contracts\near_splitter; cargo build --target wasm32-unknown-unknown --release" -ForegroundColor Yellow
}

# Check frontend dependencies
Write-Host "[>] Checking Frontend Dependencies..." -ForegroundColor Yellow
if (Test-Path "frontend\node_modules") {
    Write-Host "   [OK] Frontend dependencies installed" -ForegroundColor Green
} else {
    Write-Host "   [WARN] Frontend dependencies not installed" -ForegroundColor Yellow
    Write-Host "     Run: cd frontend; corepack pnpm install" -ForegroundColor Yellow
}

# Check .env.local
Write-Host "[>] Checking Configuration..." -ForegroundColor Yellow
if (Test-Path "frontend\.env.local") {
    $envContent = Get-Content "frontend\.env.local" -Raw
    if ($envContent -match "NEXT_PUBLIC_CONTRACT_ID=dev-\d+-\w+|NEXT_PUBLIC_CONTRACT_ID=[\w-]+\.testnet") {
        Write-Host "   [OK] .env.local configured with contract ID" -ForegroundColor Green
    } else {
        Write-Host "   [WARN] .env.local exists but contract ID looks placeholder" -ForegroundColor Yellow
        Write-Host "     Update after deploying your contract" -ForegroundColor Yellow
    }
} else {
    Write-Host "   [WARN] .env.local not found" -ForegroundColor Yellow
    Write-Host "     This will be created during deployment" -ForegroundColor Cyan
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan

if ($allGood) {
    Write-Host "[SUCCESS] All critical requirements met!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Next steps:" -ForegroundColor Cyan
    Write-Host "   1. Build the contract (if not done)" -ForegroundColor White
    Write-Host "   2. Deploy to testnet" -ForegroundColor White
    Write-Host "   3. Start the frontend" -ForegroundColor White
    Write-Host ""
    Write-Host "Quick start:" -ForegroundColor Cyan
    Write-Host "   .\scripts\deploy-testnet.ps1 -DevDeploy" -ForegroundColor White
} else {
    Write-Host "[WARNING] Some requirements missing" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Please install the missing tools above, then:" -ForegroundColor Cyan
    Write-Host "   1. Re-run this script to verify" -ForegroundColor White
    Write-Host "   2. Build and deploy the contract" -ForegroundColor White
    Write-Host "   3. Start the frontend" -ForegroundColor White
}

Write-Host ""
Write-Host "For detailed instructions, see:" -ForegroundColor Cyan
Write-Host "   QUICKSTART.md - Quick reference" -ForegroundColor White
Write-Host "   DEPLOYMENT_GUIDE.md - Step-by-step guide" -ForegroundColor White
Write-Host ""

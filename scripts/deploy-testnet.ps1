# NEAR Splitter - Testnet Deployment Script
# This script automates the deployment process to NEAR testnet

param(
    [Parameter(Mandatory=$false)]
    [string]$AccountId,
    
    [Parameter(Mandatory=$false)]
    [switch]$DevDeploy = $false,
    
    [Parameter(Mandatory=$false)]
    [switch]$SkipBuild = $false
)

$ErrorActionPreference = "Stop"

Write-Host "🚀 NEAR Splitter Testnet Deployment" -ForegroundColor Cyan
Write-Host "====================================" -ForegroundColor Cyan
Write-Host ""

# Check if Rust is installed
if (-not $SkipBuild) {
    Write-Host "📦 Step 1: Building Smart Contract..." -ForegroundColor Yellow
    
    try {
        $cargoVersion = cargo --version
        Write-Host "   ✓ Rust/Cargo found: $cargoVersion" -ForegroundColor Green
    } catch {
        Write-Host "   ✗ Error: Cargo not found!" -ForegroundColor Red
        Write-Host "   Please install Rust from https://rustup.rs/" -ForegroundColor Red
        Write-Host "   After installing, run: rustup target add wasm32-unknown-unknown" -ForegroundColor Yellow
        exit 1
    }
    
    # Check if wasm32 target is installed
    $targets = rustup target list --installed
    if ($targets -notcontains "wasm32-unknown-unknown") {
        Write-Host "   Installing wasm32-unknown-unknown target..." -ForegroundColor Yellow
        rustup target add wasm32-unknown-unknown
    }
    
    # Build the contract
    Push-Location contracts\near_splitter
    try {
        Write-Host "   Building contract..." -ForegroundColor Yellow
        cargo build --target wasm32-unknown-unknown --release
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "   ✓ Contract built successfully!" -ForegroundColor Green
        } else {
            throw "Build failed with exit code $LASTEXITCODE"
        }
    } catch {
        Write-Host "   ✗ Build failed: $_" -ForegroundColor Red
        Pop-Location
        exit 1
    }
    Pop-Location
} else {
    Write-Host "📦 Step 1: Skipping build (using existing WASM)" -ForegroundColor Yellow
}

$wasmPath = "contracts\near_splitter\target\wasm32-unknown-unknown\release\near_splitter.wasm"

# Check if WASM file exists
if (-not (Test-Path $wasmPath)) {
    Write-Host "   ✗ Error: WASM file not found at $wasmPath" -ForegroundColor Red
    Write-Host "   Run the script without -SkipBuild to build the contract first" -ForegroundColor Yellow
    exit 1
}

$wasmSize = (Get-Item $wasmPath).Length / 1KB
Write-Host "   Contract size: $([math]::Round($wasmSize, 2)) KB" -ForegroundColor Cyan
Write-Host ""

# Check if NEAR CLI is installed
Write-Host "🔧 Step 2: Checking NEAR CLI..." -ForegroundColor Yellow

try {
    $nearVersion = near --version 2>&1
    Write-Host "   ✓ NEAR CLI found" -ForegroundColor Green
} catch {
    Write-Host "   ✗ NEAR CLI not found!" -ForegroundColor Red
    Write-Host "   Install it with: npm install -g near-cli" -ForegroundColor Yellow
    Write-Host "   Or use npx: npx near-cli <command>" -ForegroundColor Yellow
    exit 1
}
Write-Host ""

# Deploy the contract
Write-Host "🚀 Step 3: Deploying Contract..." -ForegroundColor Yellow

if ($DevDeploy) {
    Write-Host "   Using dev-deploy (creates temporary dev account)..." -ForegroundColor Cyan
    near dev-deploy $wasmPath
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "   ✓ Dev deployment successful!" -ForegroundColor Green
        Write-Host ""
        Write-Host "   📝 IMPORTANT: Copy the dev account ID shown above" -ForegroundColor Yellow
        Write-Host "   and paste it into frontend\.env.local as NEXT_PUBLIC_CONTRACT_ID" -ForegroundColor Yellow
    } else {
        Write-Host "   ✗ Deployment failed" -ForegroundColor Red
        exit 1
    }
} else {
    if (-not $AccountId) {
        Write-Host "   ✗ Error: AccountId is required for named deployment" -ForegroundColor Red
        Write-Host "   Usage: .\deploy-testnet.ps1 -AccountId YOUR_ACCOUNT.testnet" -ForegroundColor Yellow
        Write-Host "   Or use: .\deploy-testnet.ps1 -DevDeploy for quick testing" -ForegroundColor Yellow
        exit 1
    }
    
    Write-Host "   Deploying to account: $AccountId" -ForegroundColor Cyan
    near deploy --accountId $AccountId --wasmFile $wasmPath
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "   ✗ Deployment failed" -ForegroundColor Red
        Write-Host "   Make sure you're logged in: near login" -ForegroundColor Yellow
        exit 1
    }
    
    Write-Host "   ✓ Contract deployed to $AccountId!" -ForegroundColor Green
    Write-Host ""
    
    # Initialize the contract
    Write-Host "🔧 Step 4: Initializing Contract..." -ForegroundColor Yellow
    near call $AccountId new '{}' --accountId $AccountId
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "   ✓ Contract initialized!" -ForegroundColor Green
    } else {
        Write-Host "   ⚠ Warning: Initialization might have failed" -ForegroundColor Yellow
        Write-Host "   The contract may already be initialized" -ForegroundColor Yellow
    }
    
    # Update .env.local
    Write-Host ""
    Write-Host "📝 Step 5: Updating Frontend Configuration..." -ForegroundColor Yellow
    
    $envPath = "frontend\.env.local"
    if (Test-Path $envPath) {
        $envContent = Get-Content $envPath -Raw
        $newContent = $envContent -replace 'NEXT_PUBLIC_CONTRACT_ID=.*', "NEXT_PUBLIC_CONTRACT_ID=$AccountId"
        Set-Content $envPath $newContent
        Write-Host "   ✓ Updated $envPath with contract ID: $AccountId" -ForegroundColor Green
    } else {
        $envContent = @"
# NEAR contract account the frontend should interact with
NEXT_PUBLIC_CONTRACT_ID=$AccountId

# Network configuration (testnet for development)
NEXT_PUBLIC_NEAR_NETWORK=testnet
"@
        Set-Content $envPath $envContent
        Write-Host "   ✓ Created $envPath with contract ID: $AccountId" -ForegroundColor Green
    }
}

Write-Host ""
Write-Host "✅ Deployment Complete!" -ForegroundColor Green
Write-Host ""
Write-Host "📋 Next Steps:" -ForegroundColor Cyan
Write-Host "   1. cd frontend" -ForegroundColor White
Write-Host "   2. corepack pnpm install (if not done)" -ForegroundColor White
Write-Host "   3. corepack pnpm dev" -ForegroundColor White
Write-Host "   4. Open http://localhost:3000" -ForegroundColor White
Write-Host ""
Write-Host "🔗 Useful Links:" -ForegroundColor Cyan
if ($DevDeploy) {
    Write-Host "   Contract Explorer: Check terminal output above for your dev account" -ForegroundColor White
} else {
    Write-Host "   Contract Explorer: https://testnet.nearblocks.io/address/$AccountId" -ForegroundColor White
}
Write-Host "   Testnet Wallet: https://testnet.mynearwallet.com" -ForegroundColor White
Write-Host "   NEAR Faucet: https://near-faucet.io (for testnet tokens)" -ForegroundColor White
Write-Host ""

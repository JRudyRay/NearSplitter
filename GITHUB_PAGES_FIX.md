# 🔧 GitHub Pages Rendering Fix

## Problem

The NearSplitter app was not rendering properly on GitHub Pages. The issue was that environment variables (`NEXT_PUBLIC_CONTRACT_ID` and `NEXT_PUBLIC_NEAR_NETWORK`) were being accessed at **runtime** in the browser, but in Next.js 15 static exports, these variables are only available during the **build process**.

## Root Cause

In `frontend/lib/env.ts`, the code was throwing an error if `process.env.NEXT_PUBLIC_CONTRACT_ID` was undefined:

```typescript
// ❌ OLD CODE - Threw error at runtime
const contractId = process.env.NEXT_PUBLIC_CONTRACT_ID;
if (!contractId) {
  throw new Error(`Missing required env var NEXT_PUBLIC_CONTRACT_ID...`);
}
```

When deployed to GitHub Pages as a static site, `process.env` is empty in the browser, causing the app to crash before rendering anything.

## Solution

Modified `frontend/lib/env.ts` to provide **fallback values** instead of throwing errors:

```typescript
// ✅ NEW CODE - Provides defaults for production
export function loadEnv(): EnvShape {
  // In Next.js 15 static exports, env vars must be inlined at build time
  // Reference them directly so they get replaced during build
  const contractId = process.env.NEXT_PUBLIC_CONTRACT_ID || 'nearsplitter-5134.testnet';
  const network = process.env.NEXT_PUBLIC_NEAR_NETWORK || 'testnet';

  return {
    NEXT_PUBLIC_CONTRACT_ID: contractId,
    NEXT_PUBLIC_NEAR_NETWORK: network,
  };
}
```

## How It Works

1. **During Build** (GitHub Actions):
   - Environment variables are set in `.github/workflows/deploy.yml`
   - Next.js replaces `process.env.NEXT_PUBLIC_*` with actual values
   - Values are **embedded** directly into the JavaScript bundles

2. **At Runtime** (Browser):
   - If env vars weren't embedded (shouldn't happen), fallbacks are used
   - The app always has valid configuration values
   - No runtime errors occur

## Verification

You can verify the fix by checking the built JavaScript:

```powershell
# Build the app
cd frontend
pnpm build

# Check that contract ID is embedded
cd out/_next/static/chunks/app
Select-String -Pattern "nearsplitter-5134.testnet" -Path *.js
```

You should see the contract ID embedded in the JavaScript files.

## Deployment

The fix is automatically applied when you push to the `main` branch:

1. **Commit and Push**:
   ```bash
   git add frontend/lib/env.ts
   git commit -m "Fix: Add fallback env vars for GitHub Pages deployment"
   git push origin main
   ```

2. **GitHub Actions** will:
   - Build with `NEXT_PUBLIC_CONTRACT_ID=nearsplitter-5134.testnet`
   - Build with `NEXT_PUBLIC_NEAR_NETWORK=testnet`
   - Embed these values into the static files
   - Deploy to GitHub Pages

3. **Visit Your Site**:
   - https://jrudyray.github.io/NearSplitter-git/
   - The app should now render correctly!

## Local Development

For local development, you still need a `.env.local` file:

```bash
# frontend/.env.local
NEXT_PUBLIC_CONTRACT_ID=nearsplitter-5134.testnet
NEXT_PUBLIC_NEAR_NETWORK=testnet
```

But now if the file is missing, the app will use fallback values instead of crashing.

## Why This Approach?

1. **No Runtime Errors**: App never crashes due to missing env vars
2. **Build-Time Embedding**: Env vars are properly inlined during build
3. **Fallback Safety**: Production deployment always has valid values
4. **Developer Friendly**: Still works with `.env.local` for development

## Related Files Changed

- ✅ `frontend/lib/env.ts` - Added fallback values
- ✅ `.github/workflows/deploy.yml` - Already had env vars configured correctly
- ✅ `frontend/next.config.js` - Already configured for static export

## Testing

To test locally:

```powershell
# Build and verify
cd frontend
pnpm build

# Serve the static files
npx serve@latest out

# Open http://localhost:3000/NearSplitter-git/ in your browser
```

The app should render correctly with the default contract ID.

---

**Status**: ✅ **FIXED** - Ready to deploy to GitHub Pages

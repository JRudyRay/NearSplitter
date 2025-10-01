# ✅ Pre-Commit Checklist

Use this checklist before committing and pushing to GitHub.

## 🧹 Clean Repository

- [x] Moved historical docs to `docs/` folder
- [x] Updated `.gitignore` to exclude build artifacts
- [x] Removed unnecessary files (should be gitignored):
  - `node_modules/`
  - `.next/`
  - `target/`
  - `*.wasm` files
  - `Cargo.lock` files

## 📝 Documentation

- [x] Updated `README.md` with comprehensive information
- [x] Created `GITHUB_PAGES_SETUP.md` deployment guide
- [x] Organized docs in `docs/` folder
- [x] All markdown files are well-formatted

## ⚙️ Configuration

- [x] `next.config.js` configured for GitHub Pages:
  - `output: 'export'`
  - `basePath: '/NearSplitter-git'`
  - `images: { unoptimized: true }`
- [x] GitHub Actions workflow created (`.github/workflows/deploy.yml`)
- [x] Environment variables documented

## 🧪 Testing

Before you commit, test locally:

```powershell
# Test frontend build
cd frontend
pnpm install
pnpm build
pnpm start  # Test the production build at http://localhost:3000

# Test linting
pnpm lint

# Run tests
pnpm test
```

- [ ] Build completes without errors
- [ ] No linting errors
- [ ] Tests pass
- [ ] App runs correctly in production mode
- [ ] Wallet connection works
- [ ] Circle creation works
- [ ] Expense tracking works

## 📦 Ready to Commit

### 1. Check Git Status

```bash
git status
```

Make sure:
- No `node_modules/` or `target/` directories
- No `.wasm` or `Cargo.lock` files (unless contracts/Cargo.lock)
- Only source code and documentation

### 2. Review Changes

```bash
git diff
```

Verify all changes are intentional.

### 3. Stage Files

```bash
# Add all files
git add .

# Or add specific files/folders
git add frontend/ contracts/ .github/ docs/ README.md
```

### 4. Commit

```bash
git commit -m "feat: prepare for GitHub Pages deployment

- Clean up repository structure
- Move historical docs to docs/ folder
- Update .gitignore for build artifacts
- Create comprehensive README
- Configure Next.js for static export
- Add GitHub Actions workflow for auto-deployment
- Add deployment documentation"
```

### 5. Push to GitHub

```bash
# Push to main branch
git push origin main

# Or if first time
git push -u origin main
```

## 🚀 After Pushing

### 1. Enable GitHub Pages

1. Go to repository **Settings**
2. Click **Pages** in sidebar
3. Under **Build and deployment**:
   - Source: `GitHub Actions`
4. Save

### 2. Watch Deployment

1. Go to **Actions** tab
2. Watch "Deploy to GitHub Pages" workflow
3. Wait ~2-3 minutes

### 3. Verify Live Site

Visit: https://jrudyray.github.io/NearSplitter-git/

Test:
- [ ] Site loads correctly
- [ ] Wallet connection works
- [ ] All features functional
- [ ] No console errors
- [ ] Mobile responsive

## 📊 Repository Structure

Your final repo should look like:

```
NearSplitter-git/
├── .github/
│   └── workflows/
│       └── deploy.yml          ✅ Auto-deployment
├── contracts/
│   ├── near_splitter/          ✅ Smart contract
│   └── test_contract/          ✅ Test contract
├── docs/                       ✅ Historical documentation
│   ├── AUTO_DISCOVERY.md
│   ├── CHANGES.md
│   ├── CONTRACT_FIX.md
│   ├── DEPLOYMENT_AUTO_DISCOVERY.md
│   ├── DEPLOYMENT_GUIDE.md
│   ├── DEPLOYMENT_SUCCESS.md
│   ├── FEATURE_UPDATE.md
│   └── REGISTRATION_FIX.md
├── frontend/                   ✅ Next.js app
│   ├── app/
│   ├── components/
│   ├── lib/
│   └── package.json
├── scripts/                    ✅ Automation scripts
├── .gitignore                  ✅ Updated
├── GITHUB_PAGES_SETUP.md       ✅ Deployment guide
├── LICENSE                     ✅ MIT License
├── package.json                ✅ Workspace config
├── QUICKSTART.md               ✅ Quick start guide
├── README.md                   ✅ Comprehensive docs
└── TESTING_GUIDE.md            ✅ Testing guide
```

## 🎯 Final Checks

Before declaring victory:

- [ ] Repository is clean (no build artifacts committed)
- [ ] Documentation is comprehensive
- [ ] GitHub Actions workflow is present
- [ ] Next.js is configured for GitHub Pages
- [ ] All tests pass locally
- [ ] Ready to push to GitHub

## 🎉 You're Ready!

Once all boxes are checked, commit and push:

```bash
git add .
git commit -m "feat: prepare repository for GitHub Pages"
git push origin main
```

Then enable GitHub Pages in repository settings and watch it deploy! 🚀

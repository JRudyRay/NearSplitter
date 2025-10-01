# 🚀 GitHub Pages Deployment Guide

This guide walks you through deploying NearSplitter to GitHub Pages.

## 📋 Prerequisites

- GitHub account with access to the repository
- Repository pushed to GitHub
- Admin access to repository settings

## ⚙️ One-Time Setup

### 1. Enable GitHub Pages

1. Go to your repository on GitHub
2. Click **Settings** (top menu)
3. Click **Pages** (left sidebar)
4. Under **Build and deployment**:
   - **Source**: Select `GitHub Actions`
5. Click **Save**

That's it! The workflow will automatically deploy on every push to `main`.

## 🔄 Automatic Deployment

Once set up, deployment happens automatically:

1. **Push to main branch**:
   ```bash
   git add .
   git commit -m "Your changes"
   git push origin main
   ```

2. **Watch the deployment**:
   - Go to **Actions** tab in GitHub
   - See the "Deploy to GitHub Pages" workflow running
   - Wait ~2-3 minutes for completion

3. **Visit your site**:
   - https://jrudyray.github.io/NearSplitter-git/

## 🛠 Manual Deployment

You can also trigger deployment manually:

1. Go to **Actions** tab
2. Click "Deploy to GitHub Pages" workflow
3. Click **Run workflow** button
4. Select branch (usually `main`)
5. Click **Run workflow**

## 📦 What Gets Deployed

The workflow:
1. ✅ Checks out the code
2. ✅ Sets up Node.js 20 and pnpm 8
3. ✅ Installs dependencies from `frontend/`
4. ✅ Builds Next.js with `pnpm build` (creates `out/` directory)
5. ✅ Uploads the `out/` folder to GitHub Pages
6. ✅ Deploys to your GitHub Pages URL

## 🔧 Configuration Details

### Next.js Configuration

The `frontend/next.config.js` is configured for static export:

```javascript
{
  output: 'export',                    // Static HTML export
  basePath: '/NearSplitter-git',       // Repository name
  images: { unoptimized: true },       // No image optimization
  trailingSlash: true,                 // Better static hosting
}
```

### Environment Variables

**Production variables** are automatically set by the build process:
- `NODE_ENV=production` (automatic in GitHub Actions)
- `basePath` is applied based on NODE_ENV

**No secrets needed!** The NEAR contract ID is public and hardcoded in the frontend.

## 🐛 Troubleshooting

### Deployment Failed

**Check the workflow logs**:
1. Go to **Actions** tab
2. Click on the failed workflow run
3. Click on the failed job
4. Read error messages

**Common issues**:

#### Build Errors
```
Error: Build failed
```
**Fix**: Test locally first:
```bash
cd frontend
pnpm install
pnpm build
```

#### Permission Errors
```
Error: insufficient permissions
```
**Fix**: 
1. Go to **Settings** → **Actions** → **General**
2. Under "Workflow permissions"
3. Select "Read and write permissions"
4. Check "Allow GitHub Actions to create and approve pull requests"

#### 404 on Deployment
```
Site loads but shows 404
```
**Fix**: Check that `basePath` in `next.config.js` matches your repository name exactly (case-sensitive).

### Assets Not Loading

If CSS/JS don't load:
1. Check browser console for 404 errors
2. Verify `basePath` is correct
3. Clear GitHub Pages cache (wait 5-10 minutes)

### Old Version Still Showing

GitHub Pages has caching:
- Wait 5-10 minutes
- Hard refresh: `Ctrl + F5` (Windows) or `Cmd + Shift + R` (Mac)
- Try incognito/private window

## 📊 Monitoring

### View Deployment Status

**Badge in README**:
```markdown
![Deployment Status](https://github.com/JRudyRay/NearSplitter-git/actions/workflows/deploy.yml/badge.svg)
```

**Actions Tab**:
- See all deployment history
- View build times
- Check logs for each step

### Analytics

Add Google Analytics or similar to track:
- Page views
- User interactions
- Wallet connections

## 🔄 Updating the Site

Simply push to `main`:

```bash
# Make your changes
git add .
git commit -m "Update feature X"
git push origin main

# Deployment starts automatically
# Check Actions tab for progress
# Site updates in ~3 minutes
```

## 🎯 Best Practices

### Before Pushing

1. **Test locally**:
   ```bash
   cd frontend
   pnpm build
   pnpm start  # Test the production build
   ```

2. **Check for errors**:
   ```bash
   pnpm lint
   pnpm test
   ```

3. **Verify contract connection**:
   - Test wallet connection
   - Test circle creation
   - Test expense adding

### After Deployment

1. ✅ Visit the live site
2. ✅ Test wallet connection
3. ✅ Test all main features
4. ✅ Check browser console for errors
5. ✅ Test on mobile devices

## 🌐 Custom Domain (Optional)

Want a custom domain like `nearsplitter.com`?

1. **Buy a domain** (Namecheap, Google Domains, etc.)

2. **Configure DNS**:
   - Add CNAME record: `www` → `jrudyray.github.io`
   - Add A records for apex domain:
     - `185.199.108.153`
     - `185.199.109.153`
     - `185.199.110.153`
     - `185.199.111.153`

3. **Update GitHub Settings**:
   - Go to **Settings** → **Pages**
   - Under "Custom domain", enter your domain
   - Check "Enforce HTTPS"

4. **Update next.config.js**:
   ```javascript
   basePath: process.env.NODE_ENV === 'production' ? '' : '',
   ```

## 📝 Summary

✅ **Automatic**: Deploys on every push to `main`  
✅ **Fast**: ~2-3 minutes per deployment  
✅ **Free**: GitHub Pages is free for public repos  
✅ **Reliable**: Backed by GitHub's infrastructure  
✅ **Simple**: No server management needed  

Your NearSplitter app is now live and will automatically update whenever you push changes! 🎉

---

**Live Site**: https://jrudyray.github.io/NearSplitter-git/  
**Repository**: https://github.com/JRudyRay/NearSplitter-git  
**Actions**: https://github.com/JRudyRay/NearSplitter-git/actions

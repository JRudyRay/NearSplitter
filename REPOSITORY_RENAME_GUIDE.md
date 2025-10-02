# 🔄 Repository Rename Guide: NearSplitter-git → NearSplitter

## ✅ Changes Already Made

I've updated all code references from `NearSplitter-git` to `NearSplitter`:

1. ✅ **README.md** - Updated demo URL, clone command, and path references
2. ✅ **next.config.js** - Changed basePath from `/NearSplitter-git` to `/NearSplitter`

## 📋 Complete Rename Process

### Step 1: Commit Current Changes

```powershell
cd c:\Users\John\Documents\GitHub\NearSplitter-git

git add .
git commit -m "chore: prepare for repository rename to NearSplitter"
git push origin main
```

### Step 2: Rename on GitHub

1. Go to https://github.com/JRudyRay/NearSplitter-git
2. Click **Settings** (top right)
3. Scroll to "Repository name"
4. Change from `NearSplitter-git` to `NearSplitter`
5. Click **Rename**

⚠️ **Important:** GitHub will show you a warning about the impact. Read it, then confirm.

### Step 3: Update Your Local Repository

```powershell
# Update the remote URL
git remote set-url origin https://github.com/JRudyRay/NearSplitter.git

# Verify the change
git remote -v
# Should show: https://github.com/JRudyRay/NearSplitter.git

# Pull to ensure everything syncs
git pull origin main
```

### Step 4: Rename Your Local Folder (Optional)

```powershell
# Go up one directory
cd ..

# Rename the folder
Rename-Item NearSplitter-git NearSplitter

# Navigate back in
cd NearSplitter

# Verify everything still works
git status
```

### Step 5: Rebuild and Redeploy

Since we changed the basePath in `next.config.js`, you need to rebuild:

```powershell
cd frontend

# Rebuild with new base path
pnpm build

# Commit the rebuilt version
cd ..
git add .
git commit -m "build: rebuild with new repository name"
git push origin main
```

### Step 6: Update GitHub Pages Settings (If Needed)

1. Go to https://github.com/JRudyRay/NearSplitter/settings/pages
2. Ensure "Source" is still set to "GitHub Actions"
3. The workflow should auto-deploy after your push

Your site will now be at: **https://jrudyray.github.io/NearSplitter/**

## ⏱️ Timeline

- **Immediately:** GitHub redirects old URLs to new ones
- **2-5 minutes:** GitHub Pages rebuilds with new URL
- **Old URL keeps working:** GitHub maintains redirects indefinitely (but update your bookmarks!)

## ✅ Verification Checklist

After completing all steps:

- [ ] Repository shows new name on GitHub
- [ ] `git remote -v` shows new URL
- [ ] Local folder renamed (optional)
- [ ] Changes committed and pushed
- [ ] GitHub Actions workflow completed successfully
- [ ] New URL works: https://jrudyray.github.io/NearSplitter/
- [ ] All features work on the new URL

## 🔗 What About Old Links?

**Good news:** GitHub automatically redirects!

- Old: `https://github.com/JRudyRay/NearSplitter-git` → Still works (redirects)
- Old: `https://jrudyray.github.io/NearSplitter-git/` → Still works (redirects)

But you should update any bookmarks, documentation, or external links to use the new name.

## 🚨 Potential Issues

### Issue: "Repository not found" after rename
**Solution:** You forgot to update the remote URL. Run:
```powershell
git remote set-url origin https://github.com/JRudyRay/NearSplitter.git
```

### Issue: GitHub Pages shows 404
**Solution:** Wait 2-5 minutes for the rebuild. Check the Actions tab for deployment status.

### Issue: Site loads but assets are broken
**Solution:** The basePath wasn't updated correctly. Verify `next.config.js` has `/NearSplitter`.

## 📝 Summary

1. ✅ Code references updated (already done)
2. ⏳ Commit and push changes
3. ⏳ Rename on GitHub
4. ⏳ Update local git remote
5. ⏳ Rebuild and redeploy
6. ✅ Enjoy your cleaner repo name!

---

**Ready?** Start with Step 1 above!

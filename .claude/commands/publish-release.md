# Publish Release

Prepare and publish an out-of-band release with AI-generated release notes (same quality as nightly).

## Usage

```
/publish-release [version]
```

- No argument: auto-derive version via `bump-build.sh` (CalVer)
- With argument: use explicit version (e.g., `2026.03.23.2-Alpha`)

---

## Workflow

**IMPORTANT:** Execute each step in order. Do not skip steps.

### Step 1: Gather Release Information

**ACTION:** Run these commands to understand what will be released:

```bash
# Get current version
cat BUILD_NUMBER

# Get the last release tag
git describe --tags --abbrev=0

# Get commits since last release
git log $(git describe --tags --abbrev=0)..HEAD --oneline --no-merges | head -20
```

### Step 2: Derive Version

```bash
# If no explicit version provided:
./scripts/bump-build.sh
cat BUILD_NUMBER

# If explicit version provided:
echo "<VERSION>" > BUILD_NUMBER
```

Then sync all manifests:
```bash
./scripts/sync-version.sh
```

### Step 3: Generate Release Notes

Use the shared release notes generator (same script as nightly CI):

```bash
TAG="v$(cat BUILD_NUMBER)"
python3 scripts/generate-release-notes.py --tag "$TAG" -o /tmp/release-notes.md
cat /tmp/release-notes.md
```

This will:
- Gather commit log and PR descriptions since the last tag
- Generate AI release notes via Copilot SDK if `COPILOT_GITHUB_TOKEN` is set
- Fall back to structured conventional-commit categorization otherwise

### Step 4: Present Release Plan to User

Show the user:
1. Current version → New version
2. The generated release notes (from `/tmp/release-notes.md`)
3. Number of commits being released

Ask: "Does this look good? Should I proceed with the release?"

**Do NOT proceed until user confirms.**

### Step 5: Run Tests

```bash
timeout 150 ./test-all.sh
```

If tests fail, stop and report the failure. Do not continue.

### Step 6: Commit, Tag, and Push

```bash
# Stage version changes
git add BUILD_NUMBER Cargo.toml Cargo.lock compiler/ori_llvm/Cargo.toml compiler/ori_rt/Cargo.toml \
        tools/ori-lsp/Cargo.toml editors/vscode-ori/package.json

# Commit with release message
TAG="v$(cat BUILD_NUMBER)"
git commit -m "chore: release $TAG"

# Create tag
git tag "$TAG"

# Push commit and tag
git push origin master --tags
```

### Step 7: Create GitHub Release with Notes

```bash
TAG="v$(cat BUILD_NUMBER)"
NOTES=$(cat /tmp/release-notes.md)

gh release create "$TAG" \
  --title "Ori ${TAG#v}" \
  --notes "$NOTES" \
  --prerelease  # Only if alpha/beta/rc
```

**NOTE:**
- Use `--prerelease` flag for alpha, beta, or rc versions
- Omit `--prerelease` for stable releases
- The tag push triggers the Release workflow which builds and attaches binaries

### Step 8: Report Success

Tell the user:
1. The release was created successfully
2. Link to the GitHub release page
3. Remind them that binaries will be built and attached automatically by CI

---

## Checklist

- [ ] Release information gathered (Step 1)
- [ ] Version derived and manifests synced (Step 2)
- [ ] Release notes generated via shared script (Step 3)
- [ ] User confirmed release plan (Step 4)
- [ ] Tests pass (Step 5)
- [ ] Changes committed and tagged (Step 6)
- [ ] GitHub release created with notes (Step 7)
- [ ] Success reported to user (Step 8)

---

## Rules

- Always get user confirmation before making any changes
- Never skip the test step
- Never force push or use destructive git operations
- Use `scripts/generate-release-notes.py` for notes — same as nightly CI
- The Release workflow (`.github/workflows/release.yml`) handles binary builds

# Creating a GitHub Release

## Prerequisites

1. Ensure all tests pass
2. Update version numbers
3. Build release binary
4. Create release assets

---

## Step 1: Prepare Release

```bash
cd agentic_server

# Run all tests
cargo test
./demo_test.sh
./test_security.sh

# Ensure clean build
cargo clean
cargo build --release

# Verify binary
./target/release/agentic-commerce-server --version
```

---

## Step 2: Create Git Tag

```bash
cd /home/dom/stateset-api

# Add all agentic_server files
git add agentic_server/

# Commit
git commit -m "Release: Agentic Commerce Server v0.4.0

- Enforce inventory reservations across checkout lifecycle
- Persist idempotent responses for replay-safe retries
- Mask payment tokens within structured logs
- Require fulfillment selection before payment readiness
- Fix OpenAI feed uploads for CSV/TSV payloads"

# Create and push tag
git tag -a agentic-server-v0.4.0 -m "Agentic Commerce Server v0.4.0"
git push origin agentic-server-v0.4.0
git push origin api-improvements
```

---

## Step 3: Create Release Assets

```bash
cd agentic_server

# Create release directory
mkdir -p release/agentic-commerce-server-v0.4.0

# Copy binary
cp target/release/agentic-commerce-server release/agentic-commerce-server-v0.4.0/

# Copy configuration files
cp docker-compose.yml release/agentic-commerce-server-v0.4.0/
cp nginx.conf release/agentic-commerce-server-v0.4.0/
cp prometheus.yml release/agentic-commerce-server-v0.4.0/
cp Dockerfile release/agentic-commerce-server-v0.4.0/
cp .env.example release/agentic-commerce-server-v0.4.0/

# Copy scripts
cp demo_test.sh release/agentic-commerce-server-v0.4.0/
cp test_security.sh release/agentic-commerce-server-v0.4.0/
cp test_e2e.sh release/agentic-commerce-server-v0.4.0/

# Copy documentation
cp README.md release/agentic-commerce-server-v0.4.0/
cp QUICK_START_PRODUCTION.md release/agentic-commerce-server-v0.4.0/
cp RELEASE_NOTES.md release/agentic-commerce-server-v0.4.0/

# Copy spec
cp ../agentic-checkout.yaml release/agentic-commerce-server-v0.4.0/

# Create tarball
cd release
tar -czf agentic-commerce-server-v0.4.0-linux-x86_64.tar.gz agentic-commerce-server-v0.4.0/
cd ..

# Create checksums
cd release
sha256sum agentic-commerce-server-v0.4.0-linux-x86_64.tar.gz > SHA256SUMS
cd ..
```

---

## Step 4: Create GitHub Release

### Via GitHub Web UI:

1. Go to https://github.com/stateset/stateset-api/releases/new

2. **Tag:** `agentic-server-v0.4.0`

3. **Title:** `Agentic Commerce Server v0.4.0 - Inventory Locking & Idempotent Replays`

4. **Description:** Copy from `RELEASE_NOTES.md`

5. **Attach Assets:**
   - `agentic-commerce-server-v0.4.0-linux-x86_64.tar.gz`
   - `SHA256SUMS`

6. Check **Set as a pre-release** (until 100% production ready)

7. Click **Publish release**

### Via GitHub CLI:

```bash
# Install gh cli if needed
# brew install gh  (Mac)
# apt install gh  (Linux)

cd /home/dom/stateset-api

# Create release
gh release create agentic-server-v0.3.0 \
  --title "Agentic Commerce Server v0.3.0" \
  --notes-file agentic_server/RELEASE_NOTES.md \
  --prerelease \
  agentic_server/release/agentic-commerce-server-v0.3.0-linux-x86_64.tar.gz \
  agentic_server/release/SHA256SUMS
```

---

## Step 5: Verify Release

```bash
# Download the release
cd /tmp
curl -L https://github.com/stateset/stateset-api/releases/download/agentic-server-v0.4.0/agentic-commerce-server-v0.4.0-linux-x86_64.tar.gz -o release.tar.gz

# Verify checksum
sha256sum -c SHA256SUMS

# Extract and test
tar -xzf release.tar.gz
cd agentic-commerce-server-v0.4.0
./agentic-commerce-server
```

---

## Release Checklist

- [ ] All tests passing (`cargo test`)
- [ ] Demo working (`./demo_test.sh`)
- [ ] Security tests passing (`./test_security.sh`)
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
- [ ] Version bumped in Cargo.toml
- [ ] Git tag created
- [ ] Release assets created
- [ ] Checksums generated
- [ ] GitHub release published
- [ ] Release announced

---

## Post-Release

1. **Announce on:**
   - GitHub Discussions
   - Twitter/X
   - Dev.to/Hashnode
   - OpenAI Community

2. **Monitor:**
   - GitHub Issues
   - Download stats
   - User feedback

3. **Plan Next Release:**
   - Review PRODUCTION_READINESS.md
   - Prioritize remaining features
   - Target 100% production ready

---

## Version History

- **v0.3.0** (Current) - Production security & services (75% ready)
- **v0.2.0** - Metrics, validation, Docker (50% ready)
- **v0.1.0** - Initial spec implementation (30% ready)

---

## Next Release (v0.4.0)

Planned features:
- Real Stripe API integration
- PostgreSQL database support
- Real-time shipping rates
- Advanced monitoring
- Load testing results
- Target: 90% production ready 

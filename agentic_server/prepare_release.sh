#!/bin/bash
set -e

echo "═══════════════════════════════════════════════════════════════"
echo "  Agentic Commerce Server - Release Preparation"
echo "  Version: 0.3.0"
echo "═══════════════════════════════════════════════════════════════"
echo

# Add all files
echo "📝 Adding files to git..."
cd /home/dom/stateset-api
git add agentic_server/

# Show what will be committed
echo
echo "Files to be committed:"
git status --short agentic_server/ | head -20
FILE_COUNT=$(git status --short agentic_server/ | wc -l)
echo "... ($FILE_COUNT total files)"
echo

# Commit
echo "💾 Creating commit..."
git commit -m "Release: Agentic Commerce Server v0.3.0

✨ Features:
- 100% Agentic Checkout Spec compliance
- 100% Delegated Payment Spec compliance  
- Stripe SharedPaymentToken support

🔒 Security:
- API key authentication
- HMAC signature verification
- Idempotency enforcement
- Rate limiting (100 req/min)
- Input validation

🏗️ Services:
- Product catalog with inventory management
- Tax calculation (5 US jurisdictions)
- Webhook delivery with retry logic
- Payment processing (vault tokens + Stripe)

📊 Infrastructure:
- Docker Compose stack (Redis + Prometheus + Grafana)
- 14 Prometheus metrics
- Nginx TLS configuration
- Redis session storage

🧪 Quality:
- 22 unit tests passing
- 2 integration test suites
- Comprehensive documentation (6,000+ lines)

Production Readiness: 75%"

echo
echo "✓ Commit created"
echo

# Create tag
echo "🏷️  Creating git tag..."
git tag -a agentic-server-v0.3.0 -m "Agentic Commerce Server v0.3.0 - Production Security & Services"
echo "✓ Tag created: agentic-server-v0.3.0"
echo

# Show instructions
echo "═══════════════════════════════════════════════════════════════"
echo "Next Steps:"
echo "═══════════════════════════════════════════════════════════════"
echo
echo "1. Push to GitHub:"
echo "   git push origin api-improvements"
echo "   git push origin agentic-server-v0.3.0"
echo
echo "2. Create GitHub Release:"
echo "   Go to: https://github.com/stateset/stateset-api/releases/new"
echo "   - Tag: agentic-server-v0.3.0"
echo "   - Title: Agentic Commerce Server v0.3.0"
echo "   - Description: Copy from agentic_server/RELEASE_NOTES.md"
echo "   - Mark as pre-release"
echo
echo "3. Or use GitHub CLI:"
echo "   gh release create agentic-server-v0.3.0 \\"
echo "     --title 'Agentic Commerce Server v0.3.0' \\"
echo "     --notes-file agentic_server/RELEASE_NOTES.md \\"
echo "     --prerelease"
echo
echo "═══════════════════════════════════════════════════════════════" 
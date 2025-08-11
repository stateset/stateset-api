# Security Policy

## Supported Versions

We release patches for security vulnerabilities. Which versions are eligible for receiving such patches depends on the CVSS v3.0 Rating:

| Version | Supported          |
| ------- | ------------------ |
| latest  | :white_check_mark: |
| < 1.0   | :x:                |

## Reporting a Vulnerability

The StateSet team takes security vulnerabilities seriously. We appreciate your efforts to responsibly disclose your findings, and will make every effort to acknowledge your contributions.

### Where to Report

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please report security vulnerabilities by emailing:
**security@stateset.com**

If possible, encrypt your message with our PGP key (available upon request).

### What to Include

Please include the following information in your report:

- Type of issue (e.g., buffer overflow, SQL injection, cross-site scripting, etc.)
- Full paths of source file(s) related to the manifestation of the issue
- The location of the affected source code (tag/branch/commit or direct URL)
- Any special configuration required to reproduce the issue
- Step-by-step instructions to reproduce the issue
- Proof-of-concept or exploit code (if possible)
- Impact of the issue, including how an attacker might exploit it

### What to Expect

- **Acknowledgment**: We will acknowledge receipt of your vulnerability report within 48 hours
- **Initial Assessment**: Within 7 days, we will provide an initial assessment and expected timeline
- **Updates**: We will keep you informed about the progress towards a fix
- **Fix**: We will notify you when the vulnerability is fixed
- **Disclosure**: We will coordinate public disclosure with you

### Security Update Process

1. The security report is received and assigned to a primary handler
2. The problem is confirmed and a list of affected versions is determined
3. Code audit is performed to find any similar problems
4. Fixes are prepared for all supported releases
5. Security advisory is prepared
6. Fixes are released and security advisory is published

## Security Best Practices for Deployment

When deploying StateSet API, please ensure:

### Environment Configuration

⚠️ **Never use default configuration values in production**

1. **Change all default credentials**:
   ```bash
   # NEVER use these defaults in production
   DATABASE_URL=postgres://postgres:postgres@localhost:5432/stateset_db  # ❌ Change this
   JWT_SECRET=your_secure_jwt_secret_key_please_change_in_production     # ❌ Change this
   ```

2. **Use strong, unique values**:
   ```bash
   # Use environment variables with strong values
   DATABASE_URL=postgres://produser:${STRONG_PASSWORD}@your-db-host:5432/stateset_prod
   JWT_SECRET=${RANDOM_256_BIT_SECRET}
   ```

3. **Secure your Redis instance**:
   - Enable Redis AUTH
   - Use TLS for Redis connections
   - Restrict network access

### API Security

1. **API Key Management**:
   - Rotate API keys regularly
   - Use different keys for different environments
   - Monitor API key usage for anomalies

2. **Rate Limiting**:
   - Configure appropriate rate limits
   - Monitor for abuse patterns
   - Implement IP-based restrictions if needed

3. **Authentication & Authorization**:
   - Enforce strong password policies
   - Implement proper session timeout
   - Use HTTPS exclusively
   - Enable CORS only for trusted domains

### Database Security

1. **Access Control**:
   - Use least-privilege database users
   - Separate read/write permissions
   - Restrict database network access

2. **Data Protection**:
   - Enable encryption at rest
   - Use TLS for database connections
   - Regular security updates

### Monitoring & Logging

1. **Security Monitoring**:
   - Monitor failed authentication attempts
   - Track API usage patterns
   - Set up alerts for suspicious activities

2. **Audit Logging**:
   - Log all administrative actions
   - Maintain logs in secure, tamper-proof storage
   - Regular log reviews

### Infrastructure Security

1. **Network Security**:
   - Use firewalls to restrict access
   - Implement network segmentation
   - Regular security scans

2. **Container Security** (if using Docker):
   - Use official base images
   - Regularly update dependencies
   - Scan images for vulnerabilities
   - Don't run containers as root

## Security Features

StateSet API includes several security features:

- **Rate Limiting**: Configurable per-endpoint rate limits
- **API Key Authentication**: Secure API key management with permissions
- **JWT Authentication**: Industry-standard token-based auth
- **RBAC**: Role-based access control for fine-grained permissions
- **Audit Logging**: Comprehensive audit trail of all actions
- **Input Validation**: Strict input validation on all endpoints
- **SQL Injection Protection**: Parameterized queries throughout

## Compliance

While StateSet API provides security features, compliance with specific standards (PCI-DSS, HIPAA, etc.) depends on your deployment and configuration. Please ensure:

- Regular security assessments
- Proper configuration following this guide
- Additional controls as required by your compliance needs

## Contact

- Security issues: **security@stateset.com**
- General support: **support@stateset.com**
- Documentation: https://docs.stateset.com

**Note**: This security policy is subject to change. Please check back regularly for updates. 
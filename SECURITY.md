# Security policy

Report a vulnerability through [GitHub private vulnerability reporting](https://github.com/srg-sphynx/FakingView/security/advisories/new)
when that option is available. Do not put API keys, access tokens, portfolio records or exploit
credentials in public issues. Include the affected version, reproducible steps and the relevant
component; remove personal data from logs and screenshots.

## Release and credential boundaries

- The initial macOS preview is **not Developer ID signed or notarized**. Download from this
  repository's release page and compare the archive against `SHA256SUMS.txt`.
- Each release includes corresponding source under GPL-3.0-only. The release workflow and
  manual publisher bind artifacts to a committed version and do not overwrite existing releases.
- Automatic update installation is disabled. No inherited maintainer signing identity is trusted.
- Kite credentials use macOS Keychain; API secrets and request tokens are transient. Never
  commit a real credential or include one in an issue, script, screenshot or social card.
- Kite integration is read-only market data. It does not submit Zerodha orders, and INR quotes
  cannot enter the USD paper-trading or portfolio asset paths.
- The optional MCP server is off by default, with explicit permissions for its operations.

CI tests use mock servers. Passing them does not establish live account authentication or data
entitlement. The dependency/workflow audit runs on repository changes and on its scheduled check.
See [the release guide](docs/releasing.md), [app specs](docs/app-specs.md) and
[third-party notices](THIRD_PARTY_NOTICES.md) for scope and provenance.

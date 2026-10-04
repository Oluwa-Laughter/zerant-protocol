# Security

Zerant is experimental protocol software, not an audited production wallet or trust
service. The native core implements signed minimal disclosure, not zero knowledge,
anonymity or unlinkability. Issuer honesty, enrollment, authenticated transport,
private-key custody, revocation watermarks and a healthy clock remain application
responsibilities. The browser demo accepts no real credential or wallet secrets.

Report sensitive vulnerabilities through this repository's GitHub **Security →
Advisories → Report a vulnerability**, if private vulnerability reporting is enabled.
If unavailable, ask maintainers to enable a private reporting channel before sharing
exploit details. No reporting email, support SLA or audit is claimed.

Only the checked-in current wire profiles are supported. Updates may change APIs;
review signed contract versions before integrating. Public RFC private-key vectors
under credential test fixtures are intentionally public test material and must never
be reused. Never commit wallet mnemonics, real issuer/holder keys or RPC secrets.

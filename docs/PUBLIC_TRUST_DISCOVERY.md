# Public trust discovery

Zerant publishes issuer-level trust information so applications can discover and validate credential authorities without receiving any holder data.

## Public endpoints

### Issuer directory

`GET /api/zerant/public/issuers?limit=24&cursor=...`

Returns a bounded cursor-paginated directory of public issuers. Each entry contains only:

- stable issuer ID;
- display name;
- whether an active signing key exists;
- number of active credential definitions;
- revocation version;
- issuer creation time.

The cursor is opaque, bounded and deterministic. The endpoint never exposes holder identifiers, recipients, credential values, wallet data or private service state.

### Issuer metadata

`GET /api/zerant/public/issuers/{issuerId}`

Returns:

- issuer ID and display name;
- public signing keys and their lifecycle state;
- immutable credential-definition versions;
- current revocation version.

Historical credential definitions remain visible so verifiers can interpret credentials that were issued under an earlier immutable version.

### Revocation publication

`GET /api/zerant/public/issuers/{issuerId}/revocation`

Returns the issuer's current signed revocation snapshot. Publications are cached in PostgreSQL and regenerated only when the revocation version, active signing key, or publication lifetime changes.

The public snapshot contains the issuer-authoritative revoked credential digests required by the credential verifier. It does not identify holders or reveal credential plaintext.

## Caching

Issuer metadata is publicly cacheable for a short bounded period. Revocation publications use a shorter cache window because they are security-sensitive freshness data.

The Next.js public proxy preserves the backend cache policy and never forwards cookies to the public endpoints.

## Privacy boundary

Public trust discovery is an organization directory, not a people directory. Public responses must never include:

- Zerant holder IDs;
- subject account IDs;
- issued credential recipients;
- credential plaintext or signed holder credentials;
- encrypted vault records or wrapped keys;
- session/authentication material;
- wallet addresses, balances, transaction history or wallet identifiers.

Regression tests serialize the public metadata shape and fail if private storage/account field names are introduced.

## Trust model

Discovery is not automatic trust. An application still decides which issuers and credential definitions it accepts. Public keys make signature verification possible; they do not make every issuer trustworthy.

Issuer key compromise state and immutable credential-definition versions remain part of the verifier's authorization policy. Revocation snapshots remain issuer-authoritative and monotonic.

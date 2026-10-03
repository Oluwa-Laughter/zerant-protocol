# Deterministic M1B fixtures

These JSON fixtures use only ASCII strings and safe integers. Their sorted compact JSON bytes are RFC 8785 compatible. `source.json`, `threshold.json` and `revocation.json` are canonical payloads; `policy.json` is the pinned public demo policy, not a reputation engine. `trust.json` authorizes the fixture issuer, claim schemas, categories, contexts and fixed policy digest.

`issuer-private.json` is **public test material**, the Ed25519 seed and public key from RFC 8032 section 7.1 test 1. Never use it for real issuance. The threshold subject public key is RFC 8032 test 2, distinct from the source key. Fixture IDs are predictable byte ranges, not production CSPRNG output. Signing does not generate IDs or establish their independence; issuers/holders must do that at enrollment.

`revocation-digest.txt` is SHA-256 of bytes 0x10 through 0x1f, encoded as unpadded base64url, computed independently with Python hashlib. The policy digest was also computed independently from the canonical policy bytes with hashlib. No fixture contains a real holder or wallet identity.

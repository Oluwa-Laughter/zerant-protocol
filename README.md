# Zerant Protocol

**Prove trust. Preserve privacy.**

Reusable atomic attestations, contextual policies and consent-bound disclosure for
people, businesses, communities, grants, marketplaces, organizations and Zcash
applications. Payment conditions are optional; open-source contributions are one
example. Contextual rules never create a universal reputation score.

## Implemented

- `zerant-core`: strict JCS encoding, safe integers, IDs, times and origin syntax.
- `zerant-credential`: Ed25519 compact JWS, scoped issuer trust, audience/expiry/key
  validity and signed revocation snapshots.
- `zerant-policy`: immutable policy parser, bounded deterministic evaluation,
  deduplication/windows and contextual evidence checks.
- `zerant-disclosure`: v0.2 signed verifier requests, one approved atomic attestation,
  holder signature, exact bindings, denial without response, SQLite replay state.
- `zerant-zcash`: native Zcash boundary with canonical ZIP-316 address inspection, ZIP-321 payment-request parsing, read-only Z3/Zallet capability/readiness discovery, bounded named-transaction verification, PCZT capability planning and minimal payment claims. Production spending remains capability-gated and is not exposed through the browser.
- `/app`: production workspace with empty integration states for holder, issuer, verifier and Zcash boundaries.
  It does not pre-populate credentials, identities, payments or verification results.
- `zerant-api`: Rust/Axum service with PostgreSQL persistence, envelope-encrypted credentials, ZecAuth RedPallas verification, HttpOnly sessions and server-only Z3/Zallet access.
- `/vault`: server-backed credential vault authenticated through ZecAuth. The browser is not the credential or session source of truth.

Enrollment/key-possession issuance, authenticated browser protocol transport, fully shielded
wallet spending, production invoice settlement and live FROST signing remain unimplemented.
No ZK, anonymity or unlinkability is claimed. Pairwise keys and metadata remain
potentially correlatable. Applications own key custody, origin authentication,
revocation watermarks, trusted clocks and issuer evidence quality.

Read [architecture](docs/ARCHITECTURE.md), [disclosure profile](docs/specs/disclosure-v0.2.md),
[privacy limits](docs/PRIVACY.md), [examples](docs/examples/README.md),
[Zcash integration](docs/ZCASH-INTEGRATION.md), [Zcash resource map](docs/ZCASH-RESOURCES.md) and [security](SECURITY.md).

## Local setup

Use Node.js **22.13+** (a supported LTS release is recommended) and pnpm **10+**. No environment variables, external services or remote fonts are required.

```sh
# With pnpm available on your PATH:
pnpm install
pnpm dev
```

Open `http://localhost:3000`, then `/app`. If pnpm is unavailable, install it using your normal package-manager setup or use `npm exec --yes --package=pnpm -- pnpm install`. This requires registry access.

Dependencies are pinned to the stable versions resolved by pnpm: Next.js 16.3.8, React 19.3.0 and Tailwind CSS 4.3.3. ESLint 9.39.2 and TypeScript 5.9.3 stay within the supported lint-tool peer ranges. `pnpm-lock.yaml` records the full graph; use frozen installs for CI and deployment.

## Development gates

Run `make check` for Rust fmt/clippy/tests, web typecheck/lint/tests/build, diff and
source secret-pattern scan. `make integration` runs native integration/test targets.
`make z3-check` probes an already running official **local regtest** router; it never
sends funds or prints raw wallet data. CI runs Rust and web gates independently.


### Rust API environment

`zerant-api` is a separate server deployment. Required production configuration:

```text
DATABASE_URL
ZERANT_PUBLIC_ORIGIN
ZERANT_VAULT_KEK_B64
ZERANT_VAULT_KEY_VERSION
ZERANT_ZCASH_CHAIN
ZERANT_ZECAUTH_SCOPES
Z3_REGTEST_RPC_ROUTER_USER
Z3_REGTEST_RPC_ROUTER_PASSWORD
```

`ZERANT_VAULT_KEK_B64` is the bootstrap key-encryption key for envelope encryption. Production custody should move behind a managed KMS/HSM. `Z3_REGTEST_*` currently drives the exercised local Z3 adapter; the current concrete router transport is intentionally regtest/read-only and is not a mainnet spending backend.

The Vercel project needs only the server-side `ZERANT_API_ORIGIN` pointing at the deployed Rust API. Do not expose it with a `NEXT_PUBLIC_` prefix.

### Vercel deployment

The deployable Next.js project lives in `apps/web`. In Vercel, set **Root Directory**
to `apps/web` once; the repository now keeps the Next.js `vercel.json`, package
metadata, favicon and build configuration inside that directory. Vercel should use its
default Next.js build/output handling. The old repository-root static-export
configuration has been removed.

Public RFC test key material is labeled under credential fixtures. Never reuse it.


## Product integration surface

The web product starts empty. `/app` exposes holder, issuer, verifier and Zcash
integration states without seeded credentials, identities, payments, request origins or
verification outcomes. `/vault` reads and writes encrypted credential records through the Rust API. Durable credentials, sessions and Zcash RPC state remain server-side.

The native v0.3 compound profile still supports one consent decision for up to eight
ordered credential, threshold and invoice-bound paid-boolean requirements; v0.2 remains
available. Every condition must verify before atomic replay consumption. Payment
recipient/amount checks stay inside the responsible native boundary; generic verifiers
receive only the approved intent-bound assertion.

See [compound disclosure](docs/specs/disclosure-v0.3.md), [payment protocol](docs/specs/payment-v0.1.md), and [Zcash integration](docs/ZCASH-INTEGRATION.md).

Native payment API: `PaymentIntent::digest/claim_context`, `Adapter::verify_intent`,
`SqlitePaymentCreditStore::register/cancel/credit_verified`, and
`payment::broadcast_state`. Public receipt JSON is an untrusted observation;
only native verified receipt tokens authorize credit. See [payment contract](docs/specs/payment-v0.1.md)
and [current capability matrix](docs/ZCASH-INTEGRATION.md).

```sh
cargo run -p zerant-zcash --example verify_payment -- --fixture < fixtures/payment-verification-input.json
make z3-check # requires Z3_REGTEST_RPC_ROUTER_PASSWORD in the environment
```

Native signatures, replay protection and payment observations run in Rust; the browser
does not invoke wallet RPC or duplicate the cryptographic verifier. PCZT and FROST
review/coordination interfaces are implemented and tested, while live spend/signing
adapters and authenticated browser protocol transport remain unimplemented. Historical
regtest captures stay test/developer evidence and are not injected into the product UI.

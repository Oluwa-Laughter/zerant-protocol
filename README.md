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
- `zerant-zcash`: read-only regtest RPC capability/readiness adapter and minimal payment-claim
  constructor. Live local discovery/readiness and a synthetic coinbase-shielding payment were exercised, including recipient/amount/confirmation checks. Fully shielded `z_sendmany` spending remains unavailable.
- `/demo`: public fixture playground across eight scenarios, with consent and honest
  integration status. Browser simulation does not execute native cryptography.

Encrypted holder vault, enrollment, authenticated browser transport, fully shielded
transfers, production invoice settlement and live FROST signing remain unimplemented.
No ZK, anonymity or unlinkability is claimed. Pairwise keys and metadata remain
potentially correlatable. Applications own key custody, origin authentication,
revocation watermarks, trusted clocks and issuer evidence quality.

Read [architecture](docs/ARCHITECTURE.md), [disclosure profile](docs/specs/disclosure-v0.2.md),
[privacy limits](docs/PRIVACY.md), [examples](docs/examples/README.md),
[Zcash integration](docs/ZCASH-INTEGRATION.md) and [security](SECURITY.md).

## Local setup

Use Node.js **22.13+** (a supported LTS release is recommended) and pnpm **10+**. No environment variables, external services or remote fonts are required.

```sh
# With pnpm available on your PATH:
pnpm install
pnpm dev
```

Open `http://localhost:3000`, then `/demo`. If pnpm is unavailable, install it using your normal package-manager setup or use `npm exec --yes --package=pnpm -- pnpm install`. This requires registry access.

Dependencies are pinned to the stable versions resolved by pnpm: Next.js 16.3.8, React 19.3.0 and Tailwind CSS 4.3.3. ESLint 9.39.2 and TypeScript 5.9.3 stay within the supported lint-tool peer ranges. `pnpm-lock.yaml` records the full graph; use frozen installs for CI and deployment.

## Development gates

Run `make check` for Rust fmt/clippy/tests, web typecheck/lint/tests/build, diff and
source secret-pattern scan. `make integration` runs native integration/test targets.
`make z3-check` probes an already running official **local regtest** router; it never
sends funds or prints raw wallet data. CI runs Rust and web gates independently.

Static web output remains `apps/web/out`. Hosting this browser playground does not
deploy a verifier or wallet service. No deployment was performed in this run.

Public RFC test key material is labeled under credential fixtures. Never reuse it.

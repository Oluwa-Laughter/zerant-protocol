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


## Compound integration console extension

The native v0.3 compound profile adds one consent decision for up to eight ordered
credential, threshold and invoice-bound paid-boolean requirements; v0.2 remains
available. Every condition must verify before atomic replay consumption. Payment
recipient/amount checks belong to the responsible local issuer; the verifier receives
only the intent-bound assertion. No source credential fallback is permitted.

The browser integration console uses public fixtures and transient state only. Its
settlement outcomes are simulations, separate from Native Rust capabilities and
previously exercised live Z3 regtest. It holds no wallet keys and invokes no spending.
Individual, freelancer, vendor, community/grant, OSS, marketplace, team and API use
cases are templates over the same protocol. PCZT discovery is per method, never
inferred from Zallet branding. Advertised spending is not authorization. Mainnet
execution, live browser wallet integration and FROST cryptography are not enabled.

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

The demo remains a public simulation with a compound request builder, consent and
settlement failure previews. Native signatures/replay and payment observations run
in Rust; the browser does not invoke a wallet or cryptographic verifier. PCZT and
FROST review/coordination interfaces are implemented and tested; live spend/signing
adapters and browser origin/vault integration remain unimplemented. Current live
reads succeeded; the latest synthetic shielding attempt failed closed. The prior
confirmed coinbase-shielding record is historical evidence, not a new fully shielded
send. Broad [composable templates](docs/examples/README.md) reuse the same primitives.

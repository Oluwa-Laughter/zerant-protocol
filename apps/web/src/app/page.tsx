import Link from "next/link";

const problems = [
  {
    title: "Stop collecting entire identities",
    body: "Applications often need one fact — eligibility, membership, role, completion or authorization — but end up collecting far more personal data than the decision requires.",
  },
  {
    title: "Make trust portable without making people trackable",
    body: "A holder can carry signed evidence between services while each verifier receives only the approved result for its own context.",
  },
  {
    title: "Keep Zcash payment evidence narrow",
    body: "A service can confirm that a required private settlement condition was satisfied without turning a wallet history into an application profile.",
  },
];

const audiences = [
  ["Individuals", "Hold credentials locally and decide what each application is allowed to learn."],
  ["Freelancers & service providers", "Prove work, eligibility or settlement conditions without exposing unrelated clients or wallet activity."],
  ["Businesses & marketplaces", "Verify vendors, customers, fulfillment or payment conditions with less data collection."],
  ["Communities & grants", "Check membership, contribution or program eligibility without building a universal reputation score."],
  ["Organizations & teams", "Use scoped roles, approvals and shared-control payment workflows without centralizing every secret."],
  ["Developers", "Integrate signed credentials, contextual policy, consent, replay protection and Zcash settlement as separate primitives."],
];

export default function Home() {
  return (
    <main id="main">
      <section className="hero product-hero">
        <div className="hero-copy">
          <p className="eyebrow"><span className="status-dot" /> Privacy-preserving trust for Zcash applications</p>
          <h1>Verify what matters.<br /><span>Keep the rest private.</span></h1>
          <p className="hero-description">
            Zerant is a credential, consent and contextual trust protocol that helps applications
            verify claims without asking users to expose their full identity, credential history
            or wallet activity.
          </p>
          <p className="muted hero-detail">
            An application asks for a specific condition. The holder reviews that request.
            Zerant verifies the approved result against trusted issuers and policy, while unrelated
            evidence stays private.
          </p>
          <div className="hero-actions">
            <Link href="/app" className="button">Open Zerant <span aria-hidden="true">→</span></Link>
            <a href="#what-zerant-solves" className="text-link">See what Zerant solves <span aria-hidden="true">↓</span></a>
          </div>
        </div>

        <div className="product-status-panel customer-summary">
          <p className="eyebrow">In one sentence</p>
          <h2>Trust checks without the data dragnet.</h2>
          <p>
            Zerant sits between an issuer, a holder and a verifier so the verifier can learn the
            required result without receiving the holder&apos;s entire private evidence.
          </p>
          <div className="product-status-row">
            <span>Instead of</span>
            <strong>Upload everything</strong>
          </div>
          <div className="product-status-row">
            <span>Zerant enables</span>
            <strong>Request one condition</strong>
          </div>
          <div className="product-status-row">
            <span>Holder controls</span>
            <strong>What gets shared</strong>
          </div>
          <div className="product-status-row">
            <span>Verifier receives</span>
            <strong>The minimum approved result</strong>
          </div>
        </div>
      </section>

      <section id="what-zerant-solves" className="section problem-section">
        <div className="section-heading">
          <p className="eyebrow">What Zerant solves</p>
          <h2>Trust should not require surveillance.</h2>
          <p className="muted">
            Zerant is designed for situations where an application needs confidence in a fact,
            not ownership of a person&apos;s whole profile.
          </p>
        </div>
        <div className="problem-grid">
          {problems.map((problem, index) => (
            <article key={problem.title}>
              <span className="eyebrow">0{index + 1}</span>
              <h3>{problem.title}</h3>
              <p>{problem.body}</p>
            </article>
          ))}
        </div>
      </section>

      <section className="section product-flow-section">
        <div className="section-heading">
          <p className="eyebrow">How Zerant works</p>
          <h2>Three roles. One narrow trust decision.</h2>
        </div>
        <div className="flow product-flow" aria-label="Zerant trust lifecycle">
          <div>
            <span className="step-number">01</span>
            <h3>Issuer creates trusted evidence</h3>
            <p>An authorized issuer signs a claim and can later revoke it.</p>
          </div>
          <span className="flow-arrow" aria-hidden="true">→</span>
          <div>
            <span className="step-number">02</span>
            <h3>Holder reviews the request</h3>
            <p>The holder sees who is asking, why, and what result will leave the private vault.</p>
          </div>
          <span className="flow-arrow" aria-hidden="true">→</span>
          <div>
            <span className="step-number">03</span>
            <h3>Verifier gets a bounded result</h3>
            <p>Zerant checks trust, expiry, revocation, audience and replay before acceptance.</p>
          </div>
        </div>
      </section>

      <section id="who-uses-zerant" className="section use-cases">
        <div className="section-heading">
          <p className="eyebrow">Who Zerant is for</p>
          <h2>One protocol, many real-world trust decisions.</h2>
          <p className="muted">
            These are applications of the same primitives — not separate versions of the protocol.
          </p>
        </div>
        <div className="use-case-grid broad-use-cases">
          {audiences.map(([title, body]) => (
            <article key={title}>
              <h3>{title}</h3>
              <p>{body}</p>
            </article>
          ))}
        </div>
      </section>

      <section id="principles" className="section principles">
        <div className="section-heading">
          <p className="eyebrow">Privacy model</p>
          <h2>Collect less by design.</h2>
          <p className="muted">Privacy is a protocol boundary, not a marketing checkbox.</p>
        </div>
        <div className="principle-list">
          <article><span className="mono muted">01</span><div><h3>Minimum disclosure</h3><p>Source credentials are not verifier fallbacks. The verifier asks for the condition it actually needs.</p></div></article>
          <article><span className="mono muted">02</span><div><h3>Explicit consent</h3><p>Origin, purpose and requirements are bound to one holder decision.</p></div></article>
          <article><span className="mono muted">03</span><div><h3>Contextual reputation</h3><p>Trust stays tied to a purpose and policy instead of becoming one universal score.</p></div></article>
          <article><span className="mono muted">04</span><div><h3>Zcash-aware settlement</h3><p>Payment evidence is verified through a separate native boundary so generic verifiers do not inherit wallet-wide visibility.</p></div></article>
        </div>
      </section>

      <section id="architecture" className="section architecture">
        <div className="section-heading">
          <p className="eyebrow">Under the hood</p>
          <h2>Clear boundaries keep private data contained.</h2>
          <p className="muted">
            Issuer trust, holder storage, verifier policy and Zcash settlement are separated so
            each component receives only what it needs.
          </p>
        </div>
        <div className="architecture-map">
          <div className="architecture-node"><span className="eyebrow">Issuer</span><h3>Signs evidence</h3><p>Authorized claims, signing keys, schemas and revocation.</p></div>
          <div className="architecture-connector">↓ <span>signed credential</span></div>
          <div className="architecture-node holder-node"><span className="eyebrow">Holder</span><h3>Stores and approves</h3><p>Encrypted local evidence and request-by-request consent.</p></div>
          <div className="architecture-connector">↓ <span>approved result</span></div>
          <div className="architecture-node"><span className="eyebrow">Verifier</span><h3>Checks the result</h3><p>Trust, validity, revocation, audience and replay checks.</p></div>
        </div>
      </section>

      <section className="section live-now">
        <div className="section-heading">
          <p className="eyebrow">Implementation status</p>
          <h2>Built, deployed, and still being connected — clearly labeled.</h2>
        </div>
        <div className="live-grid">
          <article><span className="status-badge ready">Built</span><h3>Server credential vault</h3><p>ZecAuth-authenticated credentials encrypted by the Rust API and persisted in PostgreSQL. A deployed API and database are required.</p><Link href="/vault" className="text-link">Open vault →</Link></article>
          <article><span className="status-badge ready">Built</span><h3>Native protocol core</h3><p>Credential, policy, disclosure, replay and payment verification implemented in Rust.</p></article>
          <article><span className="status-badge building">Building</span><h3>Authenticated browser transport</h3><p>The web product will receive and respond to real verifier requests through the native protocol boundary.</p></article>
          <article><span className="status-badge building">Building</span><h3>Production issuer and Zcash connections</h3><p>Issuer services, wallet operations and live threshold signing remain explicit integrations, not simulated browser state.</p></article>
        </div>
      </section>

      <section className="milestone product-cta">
        <div>
          <p className="eyebrow">Zerant workspace</p>
          <h2>Start from real state.</h2>
          <p>
            The workspace begins empty and only fills when real credentials, requests, issuer
            services or native Zcash integrations are connected.
          </p>
        </div>
        <div className="milestone-side">
          <Link href="/app" className="button">Open workspace <span aria-hidden="true">→</span></Link>
        </div>
      </section>
    </main>
  );
}

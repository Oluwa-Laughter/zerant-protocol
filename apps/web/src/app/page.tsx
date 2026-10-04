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
  ["Individuals", "Keep trusted credentials private and decide what each application is allowed to learn."],
  ["Freelancers & service providers", "Prove work, eligibility or settlement conditions without exposing unrelated clients or wallet activity."],
  ["Businesses & marketplaces", "Verify vendors, customers, fulfillment or payment conditions with less data collection."],
  ["Communities & grants", "Check membership, contribution or program eligibility without building a universal reputation score."],
  ["Organizations & teams", "Issue trusted roles and credentials, request narrow proofs, and use shared approvals for sensitive actions."],
  ["Developers", "Add private trust checks to applications without turning identity or wallet activity into a public profile."],
];

export default function Home() {
  return (
    <main id="main">
      <section className="hero product-hero">
        <div className="hero-copy">
          <p className="eyebrow"><span className="status-dot" /> Privacy-preserving trust for Zcash applications</p>
          <h1>Verify what matters.<br /><span>Keep the rest private.</span></h1>
          <p className="hero-description">
            Zerant helps people carry trusted credentials and prove specific claims without handing every application their full identity, credential history or wallet activity.
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

      <section id="how-it-works" className="section product-flow-section">
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

      <section id="use-cases" className="section use-cases">
        <div className="section-heading">
          <p className="eyebrow">Who Zerant is for</p>
          <h2>One trust layer, many real-world decisions.</h2>
          <p className="muted">
            The same privacy-first trust model can serve people, organizations, communities and applications.
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

      <section className="section architecture">
        <div className="section-heading">
          <p className="eyebrow">One private trust layer</p>
          <h2>Trust can move with you without becoming a public profile.</h2>
          <p className="muted">
            Zerant connects organizations that issue trust, people who hold it, and applications that need to verify it — while keeping unnecessary personal information out of the exchange.
          </p>
        </div>
        <div className="architecture-map">
          <div className="architecture-node"><span className="eyebrow">Issuer</span><h3>Creates trusted credentials</h3><p>Organizations attest to roles, contributions, memberships, achievements or eligibility.</p></div>
          <div className="architecture-connector">↓ <span>private credential</span></div>
          <div className="architecture-node holder-node"><span className="eyebrow">Holder</span><h3>Owns the decision</h3><p>Credentials stay private until the holder approves a specific use.</p></div>
          <div className="architecture-connector">↓ <span>approved proof</span></div>
          <div className="architecture-node"><span className="eyebrow">Verifier</span><h3>Gets only what it needs</h3><p>The application receives a narrow answer instead of the holder’s complete identity history.</p></div>
        </div>
      </section>

      <section className="section live-now">
        <div className="section-heading">
          <p className="eyebrow">What Zerant enables</p>
          <h2>A practical trust layer for people, communities and applications.</h2>
        </div>
        <div className="live-grid">
          <article><span className="status-badge ready">Available</span><h3>Receive private credentials</h3><p>Keep trusted evidence from organizations without publishing it as a public profile.</p><Link href="/vault" className="text-link">View credentials →</Link></article>
          <article><span className="status-badge ready">Available</span><h3>Issue trusted credentials</h3><p>Organizations can send private credentials directly to a recipient’s Zerant ID.</p><Link href="/issuer" className="text-link">Issuer workspace →</Link></article>
          <article><span className="status-badge ready">Available</span><h3>Prove only what matters</h3><p>Respond to verifier requests with a narrow approved result instead of sharing the whole credential.</p></article>
          <article><span className="status-badge ready">Zcash-native</span><h3>Keep wallet identity separate</h3><p>Use Zcash-powered identity and payment review without turning wallet activity into a public reputation profile.</p></article>
        </div>
      </section>

      <section className="milestone product-cta">
        <div>
          <p className="eyebrow">Zerant workspace</p>
          <h2>Your trust, on your terms.</h2>
          <p>Receive credentials, review requests and prove what matters without handing every application your full identity.</p>
        </div>
        <div className="milestone-side">
          <Link href="/app" className="button">Open workspace <span aria-hidden="true">→</span></Link>
        </div>
      </section>
    </main>
  );
}

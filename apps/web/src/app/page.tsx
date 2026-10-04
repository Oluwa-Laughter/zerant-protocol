import Link from "next/link";

export default function Home() {
  return (
    <main id="main">
      <section className="hero product-hero">
        <div className="hero-copy">
          <p className="eyebrow"><span className="status-dot" /> Privacy-preserving trust infrastructure</p>
          <h1>Prove trust.<br /><span>Preserve privacy.</span></h1>
          <p className="hero-description">
            Zerant lets applications verify the facts they need without collecting a holder&apos;s
            full identity, credential history, wallet activity, or unrelated evidence.
          </p>
          <p className="muted hero-detail">
            Credentials, contextual policy, consent, replay protection and Zcash settlement are
            independent protocol boundaries. The web product does not seed fake users or simulated
            verification results.
          </p>
          <div className="hero-actions">
            <Link href="/app" className="button">Open Zerant <span aria-hidden="true">→</span></Link>
            <Link href="/vault" className="text-link">Open local vault <span aria-hidden="true">→</span></Link>
          </div>
        </div>

        <div className="product-status-panel">
          <p className="eyebrow">Product boundaries</p>
          <div className="product-status-row">
            <span>Holder storage</span>
            <strong>Local encrypted vault</strong>
          </div>
          <div className="product-status-row">
            <span>Protocol verification</span>
            <strong>Native Rust core</strong>
          </div>
          <div className="product-status-row">
            <span>Browser request transport</span>
            <strong>Not connected</strong>
          </div>
          <div className="product-status-row">
            <span>Browser wallet authority</span>
            <strong>None</strong>
          </div>
          <p className="small muted">
            Empty states are intentional. Zerant surfaces operational data only after a real
            integration provides it.
          </p>
        </div>
      </section>

      <section className="flow" aria-label="Trust lifecycle">
        <div><span className="step-number">01</span><h3>Issuer attests</h3><p>A trusted issuer signs only claims it is authorized to make.</p></div>
        <span className="flow-arrow" aria-hidden="true">→</span>
        <div><span className="step-number">02</span><h3>Holder decides</h3><p>Private evidence stays under holder control and disclosure is request-bound.</p></div>
        <span className="flow-arrow" aria-hidden="true">→</span>
        <div><span className="step-number">03</span><h3>Verifier checks</h3><p>The verifier validates the narrow signed result against explicit trust policy.</p></div>
      </section>

      <section id="principles" className="section principles">
        <div className="section-heading">
          <p className="eyebrow">Protocol principles</p>
          <h2>Trust without building a surveillance profile.</h2>
          <p className="muted">Privacy starts with refusing to collect unrelated evidence.</p>
        </div>
        <div className="principle-list">
          <article><span className="mono muted">01</span><div><h3>Minimum disclosure</h3><p>Requests describe the exact condition required. Source credentials are not verifier fallbacks.</p></div></article>
          <article><span className="mono muted">02</span><div><h3>Explicit consent</h3><p>Origin, purpose and requirements remain bound to the holder&apos;s approval decision.</p></div></article>
          <article><span className="mono muted">03</span><div><h3>Contextual trust</h3><p>Reputation and authorization belong to a defined context, not one universal score.</p></div></article>
          <article><span className="mono muted">04</span><div><h3>Zcash as a separate boundary</h3><p>Wallet authority and payment evidence stay behind native Z3/Zallet integration rather than entering generic browser state.</p></div></article>
        </div>
      </section>

      <section id="architecture" className="section architecture">
        <div className="section-heading">
          <p className="eyebrow">Architecture</p>
          <h2>Independent roles. Narrow interfaces.</h2>
          <p className="muted">The protocol separates issuer trust, holder privacy, verifier policy and Zcash settlement.</p>
        </div>
        <div className="architecture-map">
          <div className="architecture-node"><span className="eyebrow">Issuer</span><h3>Evidence and revocation</h3><p>Authorized claims, signing keys, schemas and trust policy.</p></div>
          <div className="architecture-connector">↓ <span>signed atomic credential</span></div>
          <div className="architecture-node holder-node"><span className="eyebrow">Holder</span><h3>Encrypted vault and consent</h3><p>Private evidence remains local until an authenticated request is approved.</p></div>
          <div className="architecture-connector">↓ <span>minimum approved result</span></div>
          <div className="architecture-node"><span className="eyebrow">Verifier</span><h3>Bound verification</h3><p>Trust, validity, revocation, audience, request digest and replay checks.</p></div>
        </div>
      </section>

      <section className="milestone product-cta">
        <div>
          <p className="eyebrow">Current product</p>
          <h2>Use the real surfaces.</h2>
          <p>
            Open the workspace for live integration states or use the encrypted local vault for
            holder-controlled records. No product screen is pre-populated with fake credentials,
            payments, identities or verification outcomes.
          </p>
        </div>
        <div className="milestone-side">
          <Link href="/app" className="button">Open workspace <span aria-hidden="true">→</span></Link>
        </div>
      </section>
    </main>
  );
}

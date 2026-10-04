import type { Scenario } from "@/lib/demo-data";
import { metadata } from "@/lib/demo-data";

export function ConsentPanel({ scenario }: { scenario: Scenario }) {
  const request = scenario.request;
  const requested = request.threshold === undefined
    ? <><code>{request.claim}</code> → <strong>{String(request.result)}</strong></>
    : <><code>{request.claim}</code> · threshold ≥ {request.threshold} → <strong>{String(request.result)}</strong></>;

  return (
    <div className="consent-content">
      <div className="request-origin">
        <span className="status-dot" />
        <div>
          <span className="eyebrow">Illustrative verifier origin · not authenticated by this web preview</span>
          <p className="mono">{request.origin}</p>
        </div>
      </div>

      <dl className="request-details">
        <div><dt>Use case</dt><dd>{scenario.label} · {scenario.audience}</dd></div>
        <div><dt>Purpose</dt><dd>{request.purpose}</dd></div>
        <div><dt>Requested result</dt><dd>{requested}</dd></div>
        <div><dt>Context / policy</dt><dd>{request.context}{request.policy ? <><br />{request.policy} · operator gte</> : null}</dd></div>
        <div><dt>Selected issuer</dt><dd className="mono">{request.issuer}</dd></div>
        <div><dt>Request expiry</dt><dd>{request.requestWindow}</dd></div>
        <div><dt>Attestation expiry</dt><dd>{request.attestationWindow}</dd></div>
      </dl>

      <div className="disclosure-grid">
        <section className="shared">
          <h3>Requested & shared</h3>
          <p>{scenario.sharedSummary}</p>
        </section>
        <section className="withheld">
          <h3>Kept private</h3>
          <ul>{scenario.withheld.map((item) => <li key={item}>{item}</li>)}</ul>
        </section>
      </div>

      <details className="evidence">
        <summary>Inspect required verification metadata</summary>
        <p className="small">Category-level preview only. The Rust protocol can sign and verify these structures, but this browser playground does not hold keys or transmit credentials.</p>
        <dl>{metadata.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>
        <p className="small">Identifiers, audience keys, issuer metadata and timestamps can enable correlation. Minimal disclosure is not zero knowledge or anonymity.</p>
      </details>
    </div>
  );
}

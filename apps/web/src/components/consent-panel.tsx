import type { PreviewRequest } from "@/lib/demo-data";
import { metadata, withheld } from "@/lib/demo-data";
export function ConsentPanel({ request }: { request: PreviewRequest }) {
  return <div className="consent-content">
    <div className="request-origin"><span className="status-dot" /><div><span className="eyebrow">Illustrative verifier origin · not authenticated</span><p className="mono">{request.origin}</p></div></div>
    <dl className="request-details"><div><dt>Purpose</dt><dd>{request.purpose}</dd></div><div><dt>Requested result</dt><dd><code>{request.claim}</code> · score ≥ {request.threshold} → <strong>{String(request.result)}</strong></dd></div><div><dt>Context / policy</dt><dd>{request.context}<br />{request.policy} · operator gte</dd></div><div><dt>Selected issuer</dt><dd className="mono">{request.issuer}</dd></div><div><dt>Request expiry</dt><dd>{request.requestWindow}</dd></div><div><dt>Attestation expiry</dt><dd>{request.attestationWindow}</dd></div></dl>
    <div className="disclosure-grid"><section className="shared"><h3>Would be shared</h3><p>One issuer-attested threshold result: <strong>true</strong>, plus the metadata below. The exact score stays private.</p></section><section className="withheld"><h3>Not shared</h3><ul>{withheld.map(item => <li key={item}>{item}</li>)}</ul></section></div>
    <details className="evidence"><summary>Inspect all outbound evidence categories</summary><p className="small">This is a category-level preview, not a signed payload. No cryptographic values exist in M1A.</p><dl>{metadata.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl><p className="small">The result also includes its context, policy ID/version, threshold and operator. Identifiers, keys and timestamps are visible and can enable correlation. This is not ZK.</p></details>
  </div>;
}

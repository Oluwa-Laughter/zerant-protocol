"use client";

import { useRef, useState } from "react";
import { ConsentPanel } from "@/components/consent-panel";
import { Button } from "@/components/ui/button";
import { scenarios } from "@/lib/demo-data";
import capabilities from "../../../../fixtures/z3-regtest-discovery.json";

const roles = ["Holder", "Issuer", "Verifier"] as const;
type Role = (typeof roles)[number];

export function DemoDashboard() {
  const [role, setRole] = useState<Role>("Holder");
  const [scenarioId, setScenarioId] = useState(scenarios[0].id);
  const [decision, setDecision] = useState<"review" | "approve" | "deny" | "unavailable">("review");
  const [requirements, setRequirements] = useState<string[]>(["Credential"]);
  const [settlement, setSettlement] = useState("Unconfirmed");
  const tabs = useRef<(HTMLButtonElement | null)[]>([]);
  const scenario = scenarios.find((item) => item.id === scenarioId) ?? scenarios[0];

  function changeScenario(id: string) {
    setScenarioId(id);
    const next = scenarios.find((item) => item.id === id);
    setRequirements([next?.request.threshold === undefined ? "Credential" : "Threshold"]);
    setSettlement("Unconfirmed");
    setDecision("review");
  }

  return (
    <div className="demo">
      <div className="demo-heading">
        <p className="eyebrow">Protocol playground</p>
        <h1>One trust layer.<br />Many contexts.</h1>
        <p className="muted">Explore how the same Zerant primitives can minimize disclosure for people, businesses, communities and Zcash-powered applications.</p>
      </div>

      <div className="notice">
        <strong>Public simulation · native protocol exists separately in Rust</strong>
        <p>This page uses illustrative fixtures. It does not authenticate browser origins, unlock a vault, hold private keys, connect a wallet or send ZEC. Do not enter personal or wallet data.</p>
      </div>

      <section className="scenario-picker" aria-labelledby="scenario-heading">
        <div>
          <p className="eyebrow" id="scenario-heading">Choose a context</p>
          <p className="muted small">Changing the context resets the consent decision. The protocol stays the same.</p>
        </div>
        <select aria-label="Use-case scenario" value={scenarioId} onChange={(event) => changeScenario(event.target.value)}>
          {scenarios.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}
        </select>
      </section>

      <div className="scenario-strip" aria-label="Available scenarios">
        {scenarios.map((item) => (
          <button key={item.id} className={item.id === scenario.id ? "scenario-chip active" : "scenario-chip"} onClick={() => changeScenario(item.id)} aria-pressed={item.id === scenario.id}>
            {item.label}
          </button>
        ))}
      </div>

      <section className="request-builder" aria-labelledby="builder-heading">
        <div className="builder-heading">
          <div><p className="eyebrow">Integration console · public fixtures</p><h2 id="builder-heading">Compose one explicit request.</h2></div>
          <span className="pill">No wallet connection</span>
        </div>
        <p>One approval covers the complete selected request. Every selected condition must pass; changing a requirement resets consent.</p>
        <fieldset className="requirement-options">
          <legend>Select explicit requirements</legend>
          {["Credential", "Threshold", "Payment receipt", "Approval"].map((type) => (
            <label key={type} className="requirement-option">
              <input type="checkbox" checked={requirements.includes(type)} onChange={(event) => {
                setRequirements(event.target.checked ? [...requirements, type] : requirements.filter((item) => item !== type));
                setDecision("review");
              }} /> {type}
            </label>
          ))}
        </fieldset>
        <dl className="request-details" aria-label="Complete outbound requirement summary">
          <div><dt>Illustrative requester</dt><dd>{scenario.request.origin} · unauthenticated fixture</dd></div>
          <div><dt>Purpose</dt><dd>{scenario.request.purpose}</dd></div>
          <div><dt>Requirements</dt><dd>{requirements.length ? requirements.map((type) => type === "Threshold" ? scenario.request.threshold === undefined ? "Threshold evidence unavailable in this template" : `${scenario.request.claim}: true; ${scenario.request.policy}; threshold ${scenario.request.threshold}` : type === "Credential" ? `${scenario.request.threshold === undefined ? scenario.request.claim : "membership.active"}: ${scenario.request.threshold === undefined ? String(scenario.request.result) : "true"}` : type === "Approval" ? "approval.granted: true; illustrative context" : "payment.invoice_paid: true; opaque intent digest").join(" · ") : "Empty request · cannot approve"}</dd></div>
          <div><dt>Disclosure</dt><dd>Selected atomic results, issuer/key IDs, audience key, attestation IDs, revocation handles, validity and request bindings. This links the selected results within this request.</dd></div>
          <div><dt>Kept private</dt><dd>Exact scores, source credentials, unrelated contexts, signing shares and wallet history.</dd></div>
        </dl>
        <p className="builder-template-note">Templates include individuals, freelancers, businesses, communities, grants, OSS, marketplaces, organizations and developer/API integrations. They all reuse the same protocol.</p>
        {requirements.includes("Payment receipt") ? <section className="payment-review" aria-labelledby="payment-review">
          <h3 id="payment-review">Payment intent review</h3>
          <dl className="request-details">
            <div><dt>Amount</dt><dd>100000 zatoshis · integer fixture</dd></div>
            <div><dt>Recipient</dt><dd>Vendor checkout · redacted public fixture</dd></div>
            <div><dt>Privacy requirement</dt><dd>FullPrivacy · no automatic downgrade</dd></div>
            <div><dt>Expiry</dt><dd>300 seconds after fixture issuance</dd></div>
            <div><dt>Outbound evidence</dt><dd>Issuer-attested paid boolean bound to intent digest; no raw receipt or transaction ID</dd></div>
          </dl>
          <label className="settlement-control">Simulated settlement <select value={settlement} onChange={(event) => { setSettlement(event.target.value); setDecision("review"); }}>
            {["Unconfirmed", "Settled", "Wrong amount", "Wrong recipient", "Expired", "Stale"].map((state) => <option key={state}>{state}</option>)}
          </select></label>
        </section> : null}
        <p role="status">Fixture verifier outcome: {requirements.length === 0 ? "Rejected: empty request" : decision !== "approve" ? "No response; awaiting full-request approval" : requirements.includes("Threshold") && scenario.request.threshold === undefined ? "Rejected: threshold evidence unavailable" : requirements.includes("Payment receipt") && settlement !== "Settled" ? "Rejected: payment condition unsatisfied" : "All selected fixture conditions satisfied · simulated only"}.</p>
      </section>

      <div className="role-tabs" role="tablist" aria-label="Protocol role">
        {roles.map((item, index) => (
          <button
            key={item}
            ref={(element) => { tabs.current[index] = element; }}
            id={`tab-${item}`}
            role="tab"
            aria-selected={role === item}
            aria-controls={`panel-${item}`}
            tabIndex={role === item ? 0 : -1}
            onClick={() => setRole(item)}
            onKeyDown={(event) => {
              const next = event.key === "ArrowRight" ? (index + 1) % 3 : event.key === "ArrowLeft" ? (index + 2) % 3 : event.key === "Home" ? 0 : event.key === "End" ? 2 : -1;
              if (next >= 0) {
                event.preventDefault();
                setRole(roles[next]);
                tabs.current[next]?.focus();
              }
            }}
          >
            {item}<span className="small">0{index + 1}</span>
          </button>
        ))}
      </div>

      <div role="tabpanel" id={`panel-${role}`} aria-labelledby={`tab-${role}`} tabIndex={0} className="demo-panel">
        {role === "Holder" ? (
          <div className="holder-layout">
            <aside className="demo-sidebar">
              <p className="eyebrow">{scenario.label}</p>
              <h2>Your evidence.<br />One decision.</h2>
              <p>Private source evidence and local policy calculations should remain with the holder. The verifier receives one matching atomic attestation plus required metadata.</p>
              <div className="sidebar-note"><span className="eyebrow">No blanket permission</span><p>Unlocking a future vault does not approve a request. Denial produces no credential response.</p></div>
              {scenario.zcashStatus ? <div className="sidebar-note"><span className="eyebrow">Zcash boundary</span><p>{scenario.zcashStatus}</p></div> : null}
            </aside>
            <section className="consent-card">
              <div className="card-heading"><div><p className="eyebrow">Consent review</p><h2>Review exactly what is requested</h2></div><span className="pill">Simulation</span></div>
              <ConsentPanel scenario={scenario} />
              <div className="consent-actions">
                <p role="status" aria-live="polite">
                  {decision === "review" ? "No decision selected. Nothing is sent." : decision === "approve" ? "Approval preview selected. The web demo still signs and sends nothing." : decision === "unavailable" ? "Evidence unavailable. No false assertion or source fallback." : "Declined locally. No response or attribute data was sent."}
                </p>
                <div>
                  {decision === "review" ? <><Button variant="secondary" onClick={() => setDecision("deny")}>Decline</Button><Button variant="secondary" onClick={() => setDecision("unavailable")}>Simulate missing evidence</Button><Button disabled={requirements.length === 0} onClick={() => setDecision("approve")}>Preview approval</Button></> : <Button variant="secondary" onClick={() => setDecision("review")}>Return to review</Button>}
                </div>
              </div>
            </section>
          </div>
        ) : role === "Issuer" ? (
          <section className="role-content">
            <p className="eyebrow">Issuer · {scenario.label}</p>
            <h2>Sign only what you can substantiate.</h2>
            <p>{scenario.issuerSummary}</p>
            <dl className="request-details">
              <div><dt>Illustrative issuer</dt><dd className="mono">{scenario.request.issuer}</dd></div>
              <div><dt>Context</dt><dd>{scenario.request.context}</dd></div>
              <div><dt>Claim</dt><dd>{scenario.request.claim}{scenario.request.threshold !== undefined ? ` · supported threshold ${scenario.request.threshold}` : ""}</dd></div>
              <div><dt>Trust boundary</dt><dd>A valid signature authenticates the assertion; it does not establish issuer honesty.</dd></div>
            </dl>
          </section>
        ) : (
          <section className="role-content">
            <p className="eyebrow">Verifier · {scenario.audience}</p>
            <h2>Verify one result, not a profile.</h2>
            <p>{scenario.verifierSummary}</p>
            <dl className="request-details">
              <div><dt>Illustrative origin</dt><dd>{scenario.request.origin} · not authenticated by this preview</dd></div>
              <div><dt>Request</dt><dd>{scenario.request.claim}{scenario.request.threshold !== undefined ? ` · threshold ${scenario.request.threshold}` : ""}</dd></div>
              <div><dt>Native checks</dt><dd>Verifier signature, issuer authorization, credential signature, audience, policy binding, expiry, revocation, holder signature, origin, challenge, nonce, request digest and replay state.</dd></div>
              <div><dt>Replay state</dt><dd>The Rust disclosure crate has in-memory and SQLite atomic one-time consumption; this web page does not call it yet.</dd></div>
            </dl>
          </section>
        )}
      </div>

      <section className="integration-status" aria-label="Zcash integration status">
        <p className="eyebrow">Zcash integration</p>
        <h2>Capability first. Claims second.</h2>
        <p>The Rust adapter understands Z3 capability discovery plus minimal Zebra/Zallet readiness projections. It does not export wallet-wide data into credentials and it does not send ZEC from this web playground.</p>
        <div className="status-grid">
          <div><strong>Native Rust</strong><span>rpc.discover, getblockchaininfo, getwalletinfo projections</span></div>
          <div><strong>Recorded live RPC capabilities · historical snapshot</strong><span>{capabilities.methods.map((method) => method.name).filter((name) => ["z_sendmany", "z_sendfromaccount", "z_viewtransaction", "pczt_create"].includes(name)).join(", ")}. PCZT is absent from this historical snapshot; no live browser connection or spend readiness is established.</span></div>
          <div><strong>Previously exercised live Z3 regtest</strong><span>Recorded local regtest discovery, chain and wallet RPC checks succeeded. A synthetic coinbase-shielding payment was confirmed locally; fully shielded sends and production settlement remain unavailable.</span></div>
        </div>
      </section>
    </div>
  );
}

"use client";

import { useRef, useState } from "react";
import { ConsentPanel } from "@/components/consent-panel";
import { Button } from "@/components/ui/button";
import { scenarios } from "@/lib/demo-data";

const roles = ["Holder", "Issuer", "Verifier"] as const;
type Role = (typeof roles)[number];

export function DemoDashboard() {
  const [role, setRole] = useState<Role>("Holder");
  const [scenarioId, setScenarioId] = useState(scenarios[0].id);
  const [decision, setDecision] = useState<"review" | "approve" | "deny" | "unavailable">("review");
  const tabs = useRef<(HTMLButtonElement | null)[]>([]);
  const scenario = scenarios.find((item) => item.id === scenarioId) ?? scenarios[0];

  function changeScenario(id: string) {
    setScenarioId(id);
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
                  {decision === "review" ? <><Button variant="secondary" onClick={() => setDecision("deny")}>Decline</Button><Button variant="secondary" onClick={() => setDecision("unavailable")}>Simulate missing evidence</Button><Button onClick={() => setDecision("approve")}>Preview approval</Button></> : <Button variant="secondary" onClick={() => setDecision("review")}>Return to review</Button>}
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
          <div><strong>Implemented</strong><span>rpc.discover, getblockchaininfo, getwalletinfo projections</span></div>
          <div><strong>Detected capability</strong><span>z_sendmany can be recognized when current Z3 advertises it</span></div>
          <div><strong>Not claimed</strong><span>Recorded local regtest discovery, chain and wallet RPC checks succeeded. A synthetic coinbase-shielding payment was confirmed locally; fully shielded sends and production settlement remain unavailable.</span></div>
        </div>
      </section>
    </div>
  );
}

"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";

type AddressSummary = {
  canonical: string;
  network: string;
  kind: string;
  unified: boolean;
  can_receive_memo: boolean;
  transparent_only: boolean;
  supports_orchard: boolean;
  supports_sapling: boolean;
  supports_transparent: boolean;
};

export function ZcashAddressInspector({ enabled }: { enabled: boolean }) {
  const [address, setAddress] = useState("");
  const [summary, setSummary] = useState<AddressSummary | null>(null);
  const [status, setStatus] = useState(
    enabled
      ? "Paste a real Zcash address to validate it."
      : "Connect your Zcash identity before using address inspection.",
  );

  async function inspect() {
    setSummary(null);
    const response = await fetch("/api/zerant/zcash/address", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ address }),
    });

    if (!response.ok) {
      setStatus(
        response.status === 401
          ? "Your Zerant session is not authenticated."
          : "That address is invalid or unsupported.",
      );
      return;
    }

    const result = (await response.json()) as AddressSummary;
    setSummary(result);
    setStatus("Address validated by the native Zcash address parser.");
  }

  return (
    <section className="zcash-address-inspector" aria-labelledby="zcash-address-title">
      <div className="section-heading">
        <p className="eyebrow">Zcash address validation</p>
        <h2 id="zcash-address-title">Understand the destination before using it.</h2>
        <p className="muted">
          Zerant validates the canonical Zcash address server-side and reports safe capability
          metadata without exposing receiver bytes or wallet key material.
        </p>
      </div>

      <div className="request-review-panel">
        <label htmlFor="zcash-address">Zcash address</label>
        <input
          id="zcash-address"
          value={address}
          onChange={(event) => setAddress(event.target.value)}
          disabled={!enabled}
          spellCheck={false}
          placeholder="u1… / zs1… / t1…"
        />
        <div className="vault-actions">
          <Button disabled={!enabled || !address.trim()} onClick={inspect}>
            Validate address
          </Button>
        </div>
        <p className="vault-status neutral" role="status">{status}</p>
      </div>

      {summary ? (
        <div className="address-summary">
          <div>
            <span className="eyebrow">Canonical</span>
            <span className="mono">{summary.canonical}</span>
          </div>
          <dl>
            <div><dt>Network</dt><dd>{summary.network}</dd></div>
            <div><dt>Kind</dt><dd>{summary.kind}</dd></div>
            <div><dt>Unified</dt><dd>{summary.unified ? "Yes" : "No"}</dd></div>
            <div><dt>Memo capable</dt><dd>{summary.can_receive_memo ? "Yes" : "No"}</dd></div>
            <div><dt>Orchard</dt><dd>{summary.supports_orchard ? "Supported" : "No"}</dd></div>
            <div><dt>Sapling</dt><dd>{summary.supports_sapling ? "Supported" : "No"}</dd></div>
            <div><dt>Transparent</dt><dd>{summary.supports_transparent ? "Supported" : "No"}</dd></div>
            <div><dt>Transparent only</dt><dd>{summary.transparent_only ? "Yes" : "No"}</dd></div>
          </dl>
        </div>
      ) : null}
    </section>
  );
}

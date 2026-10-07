"use client";

import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";

type IssuerOption = {
  display_name: string;
  issuer_id: string;
};

export type PayoutDestination = {
  id: string;
  issuer_id: string;
  issuer_name: string;
  network: string;
  state: "active" | "withdrawn" | "expired";
  recipient: string | null;
  purpose: string | null;
  created_at: string;
  expires_at: string;
  withdrawn_at: string | null;
};

function stateLabel(state: PayoutDestination["state"]): string {
  if (state === "active") return "Shared privately";
  if (state === "withdrawn") return "Withdrawn";
  return "Expired";
}

export function ZcashPayoutDestinations({ enabled }: { enabled: boolean }) {
  const [issuers, setIssuers] = useState<IssuerOption[]>([]);
  const [items, setItems] = useState<PayoutDestination[]>([]);
  const [issuerId, setIssuerId] = useState("");
  const [recipient, setRecipient] = useState("");
  const [purpose, setPurpose] = useState("");
  const [status, setStatus] = useState(enabled
    ? "Share a shielded-capable Zcash receive address with one organization."
    : "Sign in to share private payout details.");
  const [busy, setBusy] = useState(false);

  const activeCount = useMemo(() => items.filter((item) => item.state === "active").length, [items]);

  useEffect(() => {
    if (!enabled) return;
    let active = true;
    void fetch("/api/zerant/zcash/payout-destinations", {
      credentials: "same-origin",
      cache: "no-store",
    }).then(async (response) => {
      if (!active) return;
      if (!response.ok) throw new Error("Private payout details could not be loaded.");
      const page = await response.json() as {
        items?: PayoutDestination[];
        organizations?: IssuerOption[];
      };
      const nextIssuers = Array.isArray(page.organizations) ? page.organizations : [];
      setItems(Array.isArray(page.items) ? page.items : []);
      setIssuers(nextIssuers);
      setIssuerId((current) =>
        nextIssuers.some((issuer) => issuer.issuer_id === current)
          ? current
          : nextIssuers[0]?.issuer_id || ""
      );
      if (nextIssuers.length === 0) {
        setStatus("No active issuer relationship is eligible for private payout sharing yet.");
      }
    }).catch(() => {
      if (active) setStatus("Private payout details could not be loaded.");
    });
    return () => { active = false; };
  }, [enabled]);

  async function share() {
    if (!enabled || busy || !issuerId || !recipient.trim() || !purpose.trim()) return;
    setBusy(true);
    setStatus("Validating and encrypting this payout destination…");
    try {
      const response = await fetch("/api/zerant/zcash/payout-destinations", {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          issuer_id: issuerId,
          recipient: recipient.trim(),
          purpose: purpose.trim(),
        }),
      });
      if (!response.ok) {
        if (response.status === 400) {
          throw new Error("Use a shielded-capable Zcash testnet receive address and a short purpose. Transparent-only or wrong-network addresses are not accepted here.");
        }
        if (response.status === 403) throw new Error("This organization no longer has an active credential relationship with your Zerant account.");
        if (response.status === 404) throw new Error("That organization is no longer available.");
        if (response.status === 409) throw new Error("An active payout destination already exists for this organization, or your active limit is reached. Withdraw the current share or wait for it to expire before sharing another.");
        if (response.status === 429) throw new Error("You are sharing payout details too quickly. Try again shortly.");
        throw new Error("Zerant could not share this payout destination.");
      }
      const created = await response.json() as PayoutDestination;
      setItems((current) => [created, ...current.filter((item) => item.id !== created.id)]);
      setRecipient("");
      setPurpose("");
      setStatus("Payout destination shared privately. It expires automatically after seven days and can be withdrawn sooner.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Payout destination could not be shared.");
    } finally {
      setBusy(false);
    }
  }

  async function withdraw(id: string) {
    if (!enabled || busy) return;
    setBusy(true);
    setStatus("Withdrawing private payout details…");
    try {
      const response = await fetch(
        "/api/zerant/zcash/payout-destinations/" + encodeURIComponent(id) + "/withdraw",
        { method: "POST", credentials: "same-origin" },
      );
      if (!response.ok) throw new Error("This payout destination could not be withdrawn.");
      const updated = await response.json() as PayoutDestination;
      setItems((current) => current.map((item) => item.id === updated.id ? updated : item));
      setStatus("Payout destination withdrawn. Zerant discarded the encrypted address and purpose from the active record.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Payout destination could not be withdrawn.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section id="zcash-private-payouts" className="zcash-invoice-manager" aria-labelledby="zcash-private-payouts-title">
      <div className="section-heading">
        <p className="eyebrow">Private payout details</p>
        <h2 id="zcash-private-payouts-title">Share where one organization can pay you.</h2>
        <p className="muted">
          Your Zerant ID stays your trust identity. This is a separate, temporary Zcash destination shared only with the selected organization.
        </p>
      </div>

      <div className="zcash-invoice-create">
        <label htmlFor="payout-organization">Organization</label>
        <select id="payout-organization" value={issuerId} onChange={(event) => setIssuerId(event.target.value)} disabled={!enabled || busy || issuers.length === 0}>
          {issuers.length === 0 ? <option value="">No eligible organizations yet</option> : null}
          {issuers.map((issuer) => <option key={issuer.issuer_id} value={issuer.issuer_id}>{issuer.display_name}</option>)}
        </select>
        <p className="small muted">Only active issuer organizations with a current, non-revoked credential relationship appear here. This keeps payout routing attached to an existing trust relationship instead of creating an open inbox.</p>

        <label htmlFor="payout-recipient">Shielded-capable Zcash testnet address</label>
        <input
          id="payout-recipient"
          value={recipient}
          onChange={(event) => setRecipient(event.target.value)}
          disabled={!enabled || busy || issuers.length === 0}
          spellCheck={false}
          autoComplete="off"
          placeholder="utest1… or ztestsapling…"
        />
        <p className="small muted">Transparent-only destinations are rejected in this privacy-preserving payout flow. Zerant never asks for your seed phrase, balance, or wallet history.</p>

        <label htmlFor="payout-purpose">Why this organization may use it</label>
        <input
          id="payout-purpose"
          value={purpose}
          onChange={(event) => setPurpose(event.target.value)}
          disabled={!enabled || busy || issuers.length === 0}
          maxLength={160}
          autoComplete="off"
          placeholder="Contributor reward, reimbursement, grant payout…"
        />
        <div className="vault-actions">
          <Button disabled={!enabled || busy || !issuerId || !recipient.trim() || purpose.trim().length < 3} onClick={() => void share()}>
            {busy ? "Working…" : "Share payout destination"}
          </Button>
        </div>
        <p className="small muted">The encrypted address and purpose expire after seven days. This does not authorize spending and does not link the wallet to your Zerant identity.</p>
      </div>

      <div className="workspace-card">
        <p className="eyebrow">Active sharing</p>
        <h3>{activeCount} active payout destination{activeCount === 1 ? "" : "s"}</h3>
        <p className="small muted">Only the selected organization&apos;s authorized operators can retrieve an active destination. Withdrawn and expired records no longer expose the address or purpose.</p>
      </div>

      <div className="zcash-invoice-list">
        {items.length ? items.map((item) => (
          <article className={"zcash-invoice-card state-" + item.state} key={item.id}>
            <div className="zcash-invoice-card-top">
              <div>
                <span className="eyebrow">{item.issuer_name}</span>
                <strong>{stateLabel(item.state)}</strong>
              </div>
              <span className="request-status">{item.network === "zcash:testnet" ? "Testnet" : "Mainnet"}</span>
            </div>
            {item.state === "active" ? (
              <>
                <p className="small muted">{item.purpose}</p>
                <p className="small muted mono invoice-recipient-preview">{item.recipient}</p>
                <p className="small muted">Expires {new Date(item.expires_at).toLocaleString()}</p>
                <div className="vault-actions wrap">
                  <Button variant="secondary" disabled={busy} onClick={() => void withdraw(item.id)}>Withdraw sharing</Button>
                </div>
              </>
            ) : (
              <p className="small muted">The private address and purpose are no longer available from this record.</p>
            )}
          </article>
        )) : (
          <div className="payment-history-empty">
            <strong>No payout details shared.</strong>
            <p className="small muted">{issuers.length
              ? "Use this only when an organization needs a destination to pay you after a trust or eligibility workflow."
              : "An active credential relationship with an issuer organization is required before payout details can be shared."}</p>
          </div>
        )}
      </div>

      {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
    </section>
  );
}

"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { Button } from "@/components/ui/button";

export type IssuerPayoutDestination = {
  id: string;
  network: string;
  state: "active" | "withdrawn" | "expired";
  recipient: string | null;
  purpose: string | null;
  created_at: string;
  expires_at: string;
  withdrawn_at: string | null;
};

export function IssuerPayoutInbox({ enabled }: { enabled: boolean }) {
  const [items, setItems] = useState<IssuerPayoutDestination[]>([]);
  const [status, setStatus] = useState(enabled
    ? "Only active, privately shared payout destinations are usable."
    : "Your organization role cannot access private payout destinations.");
  const [loading, setLoading] = useState(false);

  async function refresh(announce = false) {
    if (!enabled || loading) return;
    setLoading(true);
    try {
      const response = await fetch("/api/zerant/issuer/payout-destinations", {
        credentials: "same-origin",
        cache: "no-store",
      });
      if (!response.ok) {
        if (response.status === 403) throw new Error("Your organization role cannot access private payout destinations.");
        throw new Error("Private payout destinations could not be loaded.");
      }
      const page = await response.json() as { items?: IssuerPayoutDestination[] };
      setItems(Array.isArray(page.items) ? page.items : []);
      if (announce) setStatus("Payout inbox refreshed.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Payout inbox could not be loaded.");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    if (!enabled) return;
    const initialRefresh = window.setTimeout(() => { void refresh(false); }, 0);
    // The API scrubs expired payloads when this view is refreshed.
    return () => window.clearTimeout(initialRefresh);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled]);

  async function copyAddress(address: string) {
    try {
      await navigator.clipboard.writeText(address);
      setStatus("Receive address copied. Paste it into payment review and confirm the destination before approving in your wallet.");
    } catch {
      setStatus("Copy failed. Select the receive address manually.");
    }
  }

  const active = items.filter((item) => item.state === "active");

  return (
    <section id="issuer-private-payouts" className="issuer-section" aria-labelledby="issuer-private-payouts-title">
      <div className="section-heading">
        <p className="eyebrow">Private payout inbox</p>
        <h2 id="issuer-private-payouts-title">Pay eligible people without making their wallet address their identity.</h2>
        <p className="muted">
          Holders can temporarily share a shielded-capable Zcash destination with this organization. The address is private organization data, not a public issuer field and not part of the holder&apos;s credential.
        </p>
      </div>

      <div className="workspace-grid">
        <article className="workspace-card workspace-card-primary">
          <p className="eyebrow">Available now</p>
          <h3>{active.length} active destination{active.length === 1 ? "" : "s"}</h3>
          <p className="muted">Each active submission expires after seven days unless the holder withdraws it first.</p>
        </article>
        <article className="workspace-card">
          <p className="eyebrow">Privacy boundary</p>
          <h3>Address ≠ Zerant identity</h3>
          <p className="muted">Do not copy these destinations into CRM profiles or credential claims. Use them only for the stated payout purpose.</p>
        </article>
        <article className="workspace-card">
          <p className="eyebrow">Payment execution</p>
          <h3>Prepare separately</h3>
          <p className="muted">Copy an active destination into Zerant&apos;s payment review. The wallet still authorizes the transaction and the txid remains a separate payment record.</p>
          <Link className="text-link" href="/zcash/payments">Open payment review →</Link>
        </article>
      </div>

      <div className="zcash-invoice-list">
        {items.length ? items.map((item) => (
          <article className={"zcash-invoice-card state-" + item.state} key={item.id}>
            <div className="zcash-invoice-card-top">
              <div>
                <span className="eyebrow">Private payout reference</span>
                <strong className="mono">{item.id.slice(0, 8)}</strong>
              </div>
              <span className="request-status">{item.state === "active" ? "Private · active" : item.state}</span>
            </div>
            {item.state === "active" ? (
              <>
                <div className="public-invoice-privacy">
                  <strong>Purpose</strong>
                  <p>{item.purpose}</p>
                </div>
                <p className="small muted mono invoice-recipient-preview">{item.recipient}</p>
                <p className="small muted">Expires {new Date(item.expires_at).toLocaleString()} · {item.network === "zcash:testnet" ? "Zcash testnet" : "Zcash mainnet"}</p>
                <div className="vault-actions wrap">
                  <Button variant="secondary" onClick={() => item.recipient && void copyAddress(item.recipient)}>Copy receive address</Button>
                  <Link className="button secondary" href="/zcash/payments">Prepare payment</Link>
                </div>
              </>
            ) : (
              <p className="small muted">The holder&apos;s private address and purpose have been discarded from this record.</p>
            )}
          </article>
        )) : (
          <div className="payment-history-empty">
            <strong>No payout destinations shared with this organization.</strong>
            <p className="small muted">When a holder shares one, it appears here only for authorized organization operators.</p>
          </div>
        )}
      </div>

      <div className="vault-actions">
        <Button variant="secondary" disabled={!enabled || loading} onClick={() => void refresh(true)}>
          {loading ? "Refreshing…" : "Refresh payout inbox"}
        </Button>
      </div>
      {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
    </section>
  );
}

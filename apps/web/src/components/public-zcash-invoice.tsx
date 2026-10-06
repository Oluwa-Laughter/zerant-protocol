"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import { Button } from "@/components/ui/button";

export type PublicInvoice = {
  id: string;
  amount_zat: number;
  network: string;
  state: "open" | "cancelled" | "expired";
  expires_at: string;
  payment_uri: string | null;
  transparent_only: boolean;
};

function formatZec(zat: number): string {
  if (!Number.isSafeInteger(zat) || zat <= 0) return "Amount unavailable";
  const whole = Math.floor(zat / 100_000_000);
  const fraction = String(zat % 100_000_000).padStart(8, "0").replace(/0+$/, "");
  return whole.toLocaleString() + (fraction ? "." + fraction : "") + " ZEC";
}

export function PublicZcashInvoice({ invoice: initialInvoice }: { invoice: PublicInvoice }) {
  const [invoice, setInvoice] = useState(initialInvoice);
  const [status, setStatus] = useState("");
  const [refreshing, setRefreshing] = useState(false);
  const refreshInFlight = useRef(false);
  const open = invoice.state === "open" && Boolean(invoice.payment_uri);

  const refreshInvoice = useCallback(async (announce = false) => {
    if (refreshInFlight.current) return;
    refreshInFlight.current = true;
    setRefreshing(true);
    try {
      const response = await fetch("/api/zerant/public/zcash/invoices/" + encodeURIComponent(initialInvoice.id), { cache: "no-store" });
      if (!response.ok) {
        if (announce) setStatus("Invoice status could not be refreshed right now.");
        return;
      }
      const next = (await response.json()) as PublicInvoice;
      setInvoice(next);
      if (announce) setStatus("Invoice status refreshed.");
    } catch {
      if (announce) setStatus("Invoice status could not be refreshed right now.");
    } finally {
      refreshInFlight.current = false;
      setRefreshing(false);
    }
  }, [initialInvoice.id]);

  useEffect(() => {
    const onFocus = () => { void refreshInvoice(false); };
    const onVisibility = () => {
      if (document.visibilityState === "visible") void refreshInvoice(false);
    };
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibility);

    const expiresAt = Date.parse(initialInvoice.expires_at);
    const delay = Number.isFinite(expiresAt) ? Math.max(0, expiresAt - Date.now() + 500) : 0;
    const timeout = delay > 0 && delay <= 86_400_500
      ? window.setTimeout(() => { void refreshInvoice(false); }, delay)
      : null;

    return () => {
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibility);
      if (timeout !== null) window.clearTimeout(timeout);
    };
  }, [initialInvoice.expires_at, refreshInvoice]);

  async function copyRequest() {
    if (!invoice.payment_uri) return;
    try {
      await navigator.clipboard.writeText(invoice.payment_uri);
      setStatus("Zcash payment request copied. Review the recipient and amount in your wallet before approving.");
    } catch {
      setStatus("Copy failed. Use Open in wallet instead.");
    }
  }

  return <main id="main" className="public-invoice-page">
    <section className="public-invoice-card">
      <p className="eyebrow">Zerant · Zcash invoice</p>
      <h1>{formatZec(invoice.amount_zat)}</h1>
      <p className="muted">{invoice.network === "zcash:testnet" ? "Zcash testnet" : "Zcash mainnet"} · expires {new Date(invoice.expires_at).toLocaleString()}</p>
      <div className={`public-invoice-state state-${invoice.state}`}><span />{invoice.state === "open" ? "Open payment request" : invoice.state === "cancelled" ? "Invoice cancelled" : "Invoice expired"}</div>
      <Button variant="secondary" disabled={refreshing} onClick={() => void refreshInvoice(true)}>{refreshing ? "Refreshing…" : "Refresh invoice status"}</Button>

      {open ? <>
        {invoice.transparent_only ? <div className="public-invoice-warning"><strong>Transparent destination</strong><p>This destination reveals more information on chain. Continue only if that is acceptable to you.</p></div> : <div className="public-invoice-privacy"><strong>Shielded-capable destination</strong><p>Your wallet remains responsible for transaction approval and privacy review.</p></div>}
        <div className="vault-actions wrap">
          <a className="button" href={invoice.payment_uri ?? undefined}>Open in wallet</a>
          <Button variant="secondary" onClick={() => void copyRequest()}>Copy payment request</Button>
        </div>
        <p className="small muted">This page is a payment request, not a payment receipt. Zerant does not display the invoice owner’s account identity and does not claim payment settlement from this page.</p>
      </> : <p className="muted">This invoice no longer exposes an active Zcash payment request.</p>}
      {status ? <p className="vault-status neutral" role="status">{status}</p> : null}
      <Link className="text-link" href="/">About Zerant →</Link>
    </section>
  </main>;
}

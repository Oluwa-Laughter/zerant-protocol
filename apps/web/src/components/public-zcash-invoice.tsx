"use client";

import { useState } from "react";
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

export function PublicZcashInvoice({ invoice }: { invoice: PublicInvoice }) {
  const [status, setStatus] = useState("");
  const open = invoice.state === "open" && Boolean(invoice.payment_uri);

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

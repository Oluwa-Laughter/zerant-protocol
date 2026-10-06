"use client";

import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";

type Invoice = {
  id: string;
  recipient: string;
  amount_zat: number;
  network: string;
  state: "open" | "cancelled" | "expired";
  created_at: string;
  expires_at: string;
  cancelled_at: string | null;
  payment_uri: string | null;
};

type InvoicePage = { items: Invoice[] };

function formatZec(zat: number): string {
  if (!Number.isSafeInteger(zat) || zat <= 0) return "Amount unavailable";
  const whole = Math.floor(zat / 100_000_000);
  const fraction = String(zat % 100_000_000).padStart(8, "0").replace(/0+$/, "");
  return whole.toLocaleString() + (fraction ? "." + fraction : "") + " ZEC";
}

function stateLabel(state: Invoice["state"]): string {
  if (state === "open") return "Open";
  if (state === "cancelled") return "Cancelled";
  return "Expired";
}

export function ZcashInvoiceManager({ enabled }: { enabled: boolean }) {
  const [recipient, setRecipient] = useState("");
  const [amount, setAmount] = useState("");
  const [invoices, setInvoices] = useState<Invoice[]>([]);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState(enabled
    ? "Create an exact Zcash payment request you can share."
    : "Sign in to create Zcash invoices.");

  useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    void fetch("/api/zerant/zcash/invoices", { credentials: "same-origin", cache: "no-store", signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) throw new Error("Invoices unavailable");
        const page = (await response.json()) as InvoicePage;
        if (!controller.signal.aborted) setInvoices(page.items);
      })
      .catch(() => { if (!controller.signal.aborted) setStatus("Could not load invoices. Try reloading this page."); });
    return () => controller.abort();
  }, [enabled]);

  async function createInvoice() {
    if (!recipient.trim() || !amount.trim() || busy) return;
    setBusy(true);
    try {
      const response = await fetch("/api/zerant/zcash/invoices", {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ recipient: recipient.trim(), amount_zec: amount.trim() }),
      });
      if (!response.ok) {
        if (response.status === 429) throw new Error("You’re creating invoices too quickly. Try again in a minute.");
        throw new Error("Check the Zcash testnet recipient and amount. Zerant could not create this invoice.");
      }
      const invoice = (await response.json()) as Invoice;
      setInvoices((current) => [invoice, ...current.filter((item) => item.id !== invoice.id)]);
      setRecipient("");
      setAmount("");
      setStatus("Invoice created. Share its link only with the intended payer; the link reveals the exact payment request.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Invoice creation failed.");
    } finally {
      setBusy(false);
    }
  }

  async function cancelInvoice(id: string) {
    if (busy) return;
    setBusy(true);
    try {
      const response = await fetch("/api/zerant/zcash/invoices/" + encodeURIComponent(id) + "/cancel", {
        method: "POST",
        credentials: "same-origin",
      });
      if (!response.ok) throw new Error("This invoice could not be cancelled.");
      const invoice = (await response.json()) as Invoice;
      setInvoices((current) => current.map((item) => item.id === id ? invoice : item));
      setStatus("Invoice cancelled. Its public page no longer exposes an active payment request.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Invoice cancellation failed.");
    } finally {
      setBusy(false);
    }
  }

  async function copyShareLink(id: string) {
    const link = window.location.origin + "/pay/" + id;
    try {
      await navigator.clipboard.writeText(link);
      setStatus("Invoice link copied. Anyone with this link can see the exact amount and Zcash payment request.");
    } catch {
      setStatus("Copy failed. Open the invoice link and copy it from your browser.");
    }
  }

  return <section id="zcash-invoices" className="zcash-invoice-manager" aria-labelledby="zcash-invoice-title">
    <div className="section-heading">
      <p className="eyebrow">Request ZEC</p>
      <h2 id="zcash-invoice-title">Create a shareable Zcash invoice.</h2>
      <p className="muted">Zerant creates an exact ZIP-321 payment request and a public capability link. This is a payment request, not proof that you were paid.</p>
    </div>

    <div className="zcash-invoice-create">
      <label htmlFor="invoice-recipient">Receiving Zcash address</label>
      <input id="invoice-recipient" value={recipient} onChange={(event) => setRecipient(event.target.value)} disabled={!enabled || busy} spellCheck={false} autoComplete="off" placeholder="Zcash testnet address" />
      <label htmlFor="invoice-amount">Amount</label>
      <div className="zcash-amount-field"><input id="invoice-amount" value={amount} onChange={(event) => setAmount(event.target.value)} disabled={!enabled || busy} inputMode="decimal" autoComplete="off" placeholder="0.00" /><span>ZEC</span></div>
      <div className="vault-actions"><Button disabled={!enabled || busy || !recipient.trim() || !amount.trim()} onClick={() => void createInvoice()}>{busy ? "Working…" : "Create invoice"}</Button></div>
      <p className="small muted">Invoices expire after 24 hours. The share link does not reveal your Zerant ID, but it does reveal the invoice amount and exact Zcash request to anyone who has the link.</p>
    </div>

    {status ? <p className="vault-status neutral" role="status">{status}</p> : null}

    <div className="zcash-invoice-list">
      {invoices.length ? invoices.map((invoice) => <article className={`zcash-invoice-card state-${invoice.state}`} key={invoice.id}>
        <div className="zcash-invoice-card-top">
          <div><span className="eyebrow">Invoice</span><strong>{formatZec(invoice.amount_zat)}</strong></div>
          <span className="request-status">{stateLabel(invoice.state)}</span>
        </div>
        <p className="small muted">{invoice.network === "zcash:testnet" ? "Zcash testnet" : "Zcash mainnet"} · expires {new Date(invoice.expires_at).toLocaleString()}</p>
        <p className="small muted mono invoice-recipient-preview">{invoice.recipient}</p>
        {invoice.state === "open" ? <div className="vault-actions wrap">
          <Button variant="secondary" onClick={() => void copyShareLink(invoice.id)}>Copy share link</Button>
          <a className="button secondary" href={`/pay/${invoice.id}`} target="_blank" rel="noreferrer">Open public invoice</a>
          <Button variant="secondary" disabled={busy} onClick={() => void cancelInvoice(invoice.id)}>Cancel invoice</Button>
        </div> : null}
      </article>) : <div className="payment-history-empty"><strong>No invoices yet.</strong><p className="small muted">Create an invoice above when you want someone to send an exact Zcash amount.</p></div>}
    </div>
  </section>;
}

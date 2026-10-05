"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { ZcashWalletSelector } from "@/components/zcash-wallet-selector";
import { zip321Connector, type ZcashConnector } from "@/lib/zcash-connectors";
import { connectConnector, useZcashConnection } from "@/lib/zcash-connection";
import { directPaymentMode, paymentAction } from "@/lib/zcash-payment-connector";

type Payment = {
  index: number; recipient: string; amount_zat: number | null;
  memo_present: boolean; label: string | null; message: string | null;
  other_param_names: string[];
};
type Summary = {
  canonical_uri: string; payment_count: number; total_zat: number | null;
  payments: Payment[];
};
function formatZec(zat: number | null): string {
  if (zat === null) return "Amount chosen by sender";
  return (zat / 100_000_000).toLocaleString(undefined, { maximumFractionDigits: 8 }) + " ZEC";
}

export function ZcashPaymentRequestReview({ enabled }: { enabled: boolean }) {
  const connection = useZcashConnection();
  const [uri, setUri] = useState("");
  const [summary, setSummary] = useState<Summary | null>(null);
  const [status, setStatus] = useState(enabled
    ? "Paste a real ZIP-321 payment request to review it."
    : "Sign in with a passkey or supported Zcash authentication to review payment requests.");

  function openInWalletApp() {
    if (!summary) return;
    setStatus("Opening the complete validated ZIP-321 request in your wallet…");
    const action = paymentAction(zip321Connector(), summary);
    if (action.kind === "handoff") window.location.assign(action.uri);
  }

  async function payWithConnectedWallet(allowTransparent = false) {
    if (!summary) return;
    const wallet = connection.status === "connected" ? connection.selected : null;
    if (!wallet) {
      setStatus("Open the complete ZIP-321 request in your wallet instead."); return;
    }
    try {
      const action = paymentAction(wallet, summary, { allowTransparent });
      if (action.kind !== "direct") throw new Error("Open the complete ZIP-321 request in your wallet instead.");
      setStatus(`Review and approve the ${action.mode} payment in your wallet…`);
      const send = action.mode === "shielded" ? wallet.sendShieldedPayment! : wallet.sendTransparentPayment!;
      const txid = await send(action.payment);
      const shortTxid = txid.length > 18 ? txid.slice(0, 10) + "…" + txid.slice(-6) : txid;
      setStatus("Payment submitted by your wallet, pending settlement. Transaction " + shortTxid + ".");
    } catch (error) { setStatus(error instanceof Error ? error.message : "The wallet did not complete the payment."); }
  }

  async function selectWallet(connector: ZcashConnector) {
    if (!summary) return;
    if (connector.capabilities.has("zip321Handoff")) {
      const action = paymentAction(connector, summary);
      if (action.kind === "handoff") window.location.assign(action.uri);
      return;
    }
    try {
      await connectConnector(connector);
      setStatus("Wallet connected. Review the available payment action below.");
    } catch (error) { setStatus(error instanceof Error ? error.message : "Wallet connection failed."); }
  }

  async function inspect() {
    setSummary(null);
    const response = await fetch("/api/zerant/zcash/payment-request", {
      method: "POST", credentials: "same-origin", headers: { "content-type": "application/json" }, body: JSON.stringify({ uri }),
    });
    if (!response.ok) {
      if (response.status === 429) { setStatus("You’re doing that too quickly. Try again in a minute."); return; }
      setStatus(response.status === 401 ? "Your Zerant session is not authenticated." : "That payment request is invalid or unsupported.");
      return;
    }
    setSummary((await response.json()) as Summary);
    setStatus("Payment request validated by the native Zcash parser.");
  }

  const mode = summary && connection.status === "connected" ? directPaymentMode(summary, connection.selected) : null;
  return <section id="zcash-payment-review" className="zcash-request-review" aria-labelledby="zcash-request-title">
    <div className="section-heading"><p className="eyebrow">Zcash payment request</p><h2 id="zcash-request-title">Review before wallet approval.</h2>
      <p className="muted">Zerant validates the request and shows its payment details before you approve anything.</p></div>
    <div className="request-review-panel"><label htmlFor="zcash-payment-uri">Zcash payment request URI</label>
      <textarea id="zcash-payment-uri" value={uri} onChange={(event) => setUri(event.target.value)} disabled={!enabled} spellCheck={false} rows={5} placeholder="zcash:..." />
      <div className="vault-actions"><Button disabled={!enabled || !uri.trim()} onClick={inspect}>Validate request</Button></div>
      <p className="vault-status neutral" role="status">{status}</p></div>
    {summary ? <div className="request-summary"><div className="request-summary-top"><div><span className="eyebrow">Canonical request</span><span className="mono">{summary.canonical_uri}</span></div><div><span className="eyebrow">Total</span><strong>{formatZec(summary.total_zat)}</strong></div></div>
      <div className="payment-request-actions"><Button onClick={openInWalletApp}>Open complete request in wallet</Button>
        {mode === "shielded" ? <Button variant="secondary" onClick={() => void payWithConnectedWallet()}>Pay shielded with connected wallet</Button> : null}
        {mode === "transparent" ? <Button variant="secondary" onClick={() => void payWithConnectedWallet(true)}>Pay transparently with connected wallet</Button> : null}
        {!mode ? <ZcashWalletSelector purpose="payment" triggerLabel="Choose a payment wallet" onSelect={(connector) => void selectWallet(connector)} /> : null}</div>
      <p className="small muted payment-request-note">The complete ZIP-321 request remains available for wallet handoff. Direct payment appears only when the connected wallet can represent this exact simple request. Transparent payment reveals more on chain and requires your explicit choice. Submission is not settlement verification.</p>
      <div className="request-payment-list">{summary.payments.map((payment) => <article key={payment.index} className="request-payment"><div><span className="eyebrow">Recipient {payment.index + 1}</span><span className="mono">{payment.recipient}</span></div><dl><div><dt>Amount</dt><dd>{formatZec(payment.amount_zat)}</dd></div><div><dt>Label</dt><dd>{payment.label ?? "None"}</dd></div><div><dt>Message</dt><dd>{payment.message ?? "None"}</dd></div><div><dt>Memo</dt><dd>{payment.memo_present ? "Present" : "None"}</dd></div></dl></article>)}</div>
    </div> : null}
  </section>;
}

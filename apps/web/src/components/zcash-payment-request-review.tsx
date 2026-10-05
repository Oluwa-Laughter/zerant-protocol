"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { ZcashWalletSelector } from "@/components/zcash-wallet-selector";
import { zip321Connector, type ZcashConnector } from "@/lib/zcash-connectors";
import { connectConnector, useZcashConnection } from "@/lib/zcash-connection";
import { directPaymentMode, paymentAction } from "@/lib/zcash-payment-connector";

type Payment = {
  index: number;
  recipient: string;
  amount_zat: number | null;
  memo_present: boolean;
  transparent_only: boolean;
  can_receive_memo: boolean;
  label: string | null;
  message: string | null;
  other_param_names: string[];
};
type Summary = {
  canonical_uri: string;
  payment_count: number;
  total_zat: number | null;
  payments: Payment[];
};
type Flow = "send" | "review";

function formatZec(zat: number | null): string {
  if (zat === null) return "Amount chosen by sender";
  return (zat / 100_000_000).toLocaleString(undefined, { maximumFractionDigits: 8 }) + " ZEC";
}

export function ZcashPaymentRequestReview({ enabled }: { enabled: boolean }) {
  const connection = useZcashConnection();
  const [flow, setFlow] = useState<Flow>("send");
  const [recipient, setRecipient] = useState("");
  const [amount, setAmount] = useState("");
  const [uri, setUri] = useState("");
  const [summary, setSummary] = useState<Summary | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState(enabled
    ? "Enter a Zcash recipient and amount to prepare a private payment."
    : "Sign in to prepare or review Zcash payments.");

  function reset(next: Flow) {
    setFlow(next);
    setSummary(null);
    setStatus(next === "send"
      ? "Enter a Zcash recipient and amount to prepare a private payment."
      : "Paste a Zcash payment request to review it before opening your wallet.");
  }

  function openInWalletApp() {
    if (!summary) return;
    setStatus("Opening the complete validated payment in your wallet…");
    const action = paymentAction(zip321Connector(), summary);
    if (action.kind === "handoff") window.location.assign(action.uri);
  }

  async function payWithConnectedWallet(allowTransparent = false) {
    if (!summary) return;
    const wallet = connection.status === "connected" ? connection.selected : null;
    if (!wallet) {
      setStatus("Choose a compatible payment wallet first.");
      return;
    }
    try {
      const action = paymentAction(wallet, summary, { allowTransparent });
      if (action.kind !== "direct") throw new Error("Open the complete payment request in your wallet instead.");
      setStatus(`Review and approve the ${action.mode} payment in your wallet…`);
      const send = action.mode === "shielded" ? wallet.sendShieldedPayment! : wallet.sendTransparentPayment!;
      const txid = await send(action.payment);
      const shortTxid = txid.length > 18 ? txid.slice(0, 10) + "…" + txid.slice(-6) : txid;
      setStatus("Submitted by your wallet. Zerant is waiting for network settlement. Transaction " + shortTxid + ".");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "The wallet did not complete the payment.");
    }
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
      setStatus("Wallet connected. Review the payment and approve only when you are ready.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Wallet connection failed.");
    }
  }

  async function requestSummary(path: string, body: object, success: string) {
    setBusy(true);
    setSummary(null);
    try {
      const response = await fetch(path, {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
      });
      if (!response.ok) {
        if (response.status === 429) throw new Error("You’re doing that too quickly. Try again in a minute.");
        if (response.status === 401) throw new Error("Your Zerant session is not authenticated.");
        throw new Error(flow === "send"
          ? "Check the recipient, network, and ZEC amount. Zerant could not prepare this payment."
          : "That payment request is invalid or unsupported.");
      }
      const reviewed = (await response.json()) as Summary;
      setSummary(reviewed);
      setUri(reviewed.canonical_uri);
      setStatus(success);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Zerant could not prepare this payment.");
    } finally {
      setBusy(false);
    }
  }

  const createPayment = () => requestSummary(
    "/api/zerant/zcash/payment-request/create",
    { recipient: recipient.trim(), amount_zec: amount.trim() },
    "Payment prepared and validated. Review the privacy details before wallet approval.",
  );
  const inspectPayment = () => requestSummary(
    "/api/zerant/zcash/payment-request",
    { uri },
    "Payment request validated. Review it before wallet approval.",
  );

  const mode = summary && connection.status === "connected"
    ? directPaymentMode(summary, connection.selected)
    : null;
  const payment = summary?.payments[0] ?? null;

  return <section id="zcash-payment-review" className="zcash-request-review" aria-labelledby="zcash-request-title">
    <div className="section-heading">
      <p className="eyebrow">Zcash payments</p>
      <h2 id="zcash-request-title">Prepare, review, then approve in your wallet.</h2>
      <p className="muted">Zerant validates the destination and payment before your wallet is asked to move funds. Your wallet remains in control.</p>
    </div>

    <div className="vault-actions wrap" role="group" aria-label="Zcash payment flow">
      <Button variant={flow === "send" ? "primary" : "secondary"} onClick={() => reset("send")}>Send ZEC</Button>
      <Button variant={flow === "review" ? "primary" : "secondary"} onClick={() => reset("review")}>Review a payment request</Button>
    </div>

    <div className="request-review-panel">
      {flow === "send" ? <>
        <div className="section-heading compact">
          <p className="eyebrow">Private payment</p>
          <h3>Where are you sending ZEC?</h3>
        </div>
        <label htmlFor="zcash-payment-recipient">Recipient</label>
        <input id="zcash-payment-recipient" value={recipient} onChange={(event) => setRecipient(event.target.value)} disabled={!enabled || busy} spellCheck={false} autoComplete="off" placeholder="Zcash address" />
        <label htmlFor="zcash-payment-amount">Amount</label>
        <div className="zcash-amount-field">
          <input id="zcash-payment-amount" value={amount} onChange={(event) => setAmount(event.target.value)} disabled={!enabled || busy} inputMode="decimal" autoComplete="off" placeholder="0.00" />
          <span>ZEC</span>
        </div>
        <div className="vault-actions"><Button disabled={!enabled || busy || !recipient.trim() || !amount.trim()} onClick={() => void createPayment()}>{busy ? "Preparing…" : "Review payment"}</Button></div>
      </> : <>
        <label htmlFor="zcash-payment-uri">Zcash payment request</label>
        <textarea id="zcash-payment-uri" value={uri} onChange={(event) => setUri(event.target.value)} disabled={!enabled || busy} spellCheck={false} rows={5} placeholder="zcash:..." />
        <div className="vault-actions"><Button disabled={!enabled || busy || !uri.trim()} onClick={() => void inspectPayment()}>{busy ? "Checking…" : "Validate request"}</Button></div>
      </>}
      <p className="vault-status neutral" role="status">{status}</p>
    </div>

    {summary && payment ? <div className="request-summary zcash-payment-confirmation">
      <div className="request-summary-top">
        <div><span className="eyebrow">Recipient</span><span className="mono">{payment.recipient}</span></div>
        <div><span className="eyebrow">Amount</span><strong>{formatZec(summary.total_zat)}</strong></div>
      </div>

      <div className={payment.transparent_only ? "zcash-privacy-review caution" : "zcash-privacy-review protected"}>
        <div>
          <span className="eyebrow">Privacy review</span>
          <strong>{payment.transparent_only ? "Transparent destination" : "Shielded-capable destination"}</strong>
        </div>
        <p>{payment.transparent_only
          ? "This destination uses Zcash’s transparent pool. Sending to it reveals more transaction information on chain, so Zerant will never choose transparent payment for you."
          : "This destination can receive a privacy-preserving Zcash payment. Zerant will prefer a shielded wallet action when your connected wallet supports it."}</p>
      </div>

      <div className="payment-request-actions">
        {mode === "shielded" ? <Button onClick={() => void payWithConnectedWallet()}>Approve shielded payment</Button> : null}
        {mode === "transparent" ? <Button onClick={() => void payWithConnectedWallet(true)}>Approve transparent payment</Button> : null}
        {!mode ? <ZcashWalletSelector purpose="payment" triggerLabel="Choose payment wallet" onSelect={(connector) => void selectWallet(connector)} /> : null}
        <Button variant="secondary" onClick={openInWalletApp}>Open in another wallet</Button>
      </div>
      <p className="small muted payment-request-note">Wallet submission is only the first step. Zerant does not treat a submitted transaction as settled until it can be observed and confirmed on the Zcash network.</p>
      {flow === "review" && summary.payments.length > 1 ? <div className="request-payment-list">{summary.payments.map((item) => <article key={item.index} className="request-payment"><div><span className="eyebrow">Recipient {item.index + 1}</span><span className="mono">{item.recipient}</span></div><dl><div><dt>Amount</dt><dd>{formatZec(item.amount_zat)}</dd></div><div><dt>Label</dt><dd>{item.label ?? "None"}</dd></div><div><dt>Message</dt><dd>{item.message ?? "None"}</dd></div><div><dt>Memo</dt><dd>{item.memo_present ? "Present" : "None"}</dd></div></dl></article>)}</div> : null}
    </div> : null}
  </section>;
}

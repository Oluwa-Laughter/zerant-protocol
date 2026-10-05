"use client";

import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { ZcashWalletSelector } from "@/components/zcash-wallet-selector";
import { zip321Connector, type ZcashConnector } from "@/lib/zcash-connectors";
import { connectConnector, ensureZcashConfig, useZcashConnection } from "@/lib/zcash-connection";
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
type PaymentRecord = {
  id: string;
  recipient: string;
  amount_zat: number;
  network: string;
  state: "prepared" | "submitted" | "expired";
  txid: string | null;
  created_at: string;
  expires_at: string;
  submitted_at: string | null;
  payment_uri: string | null;
  transparent_only: boolean;
};
type PaymentPage = { items: PaymentRecord[]; next_cursor: string | null };

function canTrack(summary: Summary): boolean {
  const payment = summary.payments[0];
  return summary.payment_count === 1 && Boolean(payment) &&
    payment.amount_zat !== null && payment.amount_zat > 0 &&
    !payment.memo_present && payment.label === null && payment.message === null &&
    payment.other_param_names.length === 0;
}

function formatZec(zat: number | null): string {
  if (zat === null) return "Amount chosen by sender";
  if (!Number.isSafeInteger(zat) || zat < 0) return "Amount unavailable";
  const whole = Math.floor(zat / 100_000_000);
  const fraction = String(zat % 100_000_000).padStart(8, "0").replace(/0+$/, "");
  return whole.toLocaleString() + (fraction ? "." + fraction : "") + " ZEC";
}

export function ZcashPaymentRequestReview({ enabled }: { enabled: boolean }) {
  const connection = useZcashConnection();
  const [flow, setFlow] = useState<Flow>("send");
  const [recipient, setRecipient] = useState("");
  const [amount, setAmount] = useState("");
  const [uri, setUri] = useState("");
  const [summary, setSummary] = useState<Summary | null>(null);
  const [record, setRecord] = useState<PaymentRecord | null>(null);
  const [records, setRecords] = useState<PaymentRecord[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [txidDrafts, setTxidDrafts] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState(enabled
    ? "Enter a Zcash recipient and amount to prepare a private payment."
    : "Sign in to prepare or review Zcash payments.");

  const refreshRecords = useCallback(async () => {
    const response = await fetch("/api/zerant/zcash/payments", { credentials: "same-origin", cache: "no-store" });
    if (!response.ok) throw new Error("Could not load payments");
    const page = (await response.json()) as PaymentPage;
    setRecords(page.items);
    setNextCursor(page.next_cursor);
  }, []);

  async function loadMore() {
    if (!nextCursor) return;
    try {
      const response = await fetch("/api/zerant/zcash/payments?cursor=" + encodeURIComponent(nextCursor), { credentials: "same-origin", cache: "no-store" });
      if (!response.ok) throw new Error("Payment history unavailable");
      const page = (await response.json()) as PaymentPage;
      setRecords((current) => [...current, ...page.items]);
      setNextCursor(page.next_cursor);
    } catch {
      setStatus("Could not load more payments. Try again.");
    }
  }

  useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    void fetch("/api/zerant/zcash/payments", { credentials: "same-origin", cache: "no-store", signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) throw new Error("Payment history unavailable");
        const page = (await response.json()) as PaymentPage;
        if (!controller.signal.aborted) {
          setRecords(page.items);
          setNextCursor(page.next_cursor);
        }
      })
      .catch(() => { if (!controller.signal.aborted) setStatus("Could not load saved payments. Try reloading this page."); });
    return () => controller.abort();
  }, [enabled]);

  function reset(next: Flow) {
    setFlow(next);
    setSummary(null);
    setRecord(null);
    setStatus(next === "send"
      ? "Enter a Zcash recipient and amount to prepare a private payment."
      : "Paste a Zcash payment request to review it before opening your wallet.");
  }

  function openInWalletApp() {
    if (!summary || record?.state === "submitted") return;
    setStatus("Opening the complete validated payment in your wallet…");
    const action = paymentAction(zip321Connector(), summary);
    if (action.kind === "handoff") window.location.assign(action.uri);
  }

  async function copyPaymentLink() {
    if (!summary || record?.state === "submitted") return;
    try {
      await navigator.clipboard.writeText(summary.canonical_uri);
      setStatus("Payment link copied. Open it in a compatible Zcash wallet on the same network and check the recipient and amount before approving.");
    } catch {
      setStatus("Copy failed. Select the payment link shown below and copy it manually.");
    }
  }

  function reopenSavedPayment(uri: string | null) {
    if (!uri?.startsWith("zcash:")) return;
    window.location.assign(uri);
  }

  async function copySavedPayment(uri: string | null) {
    if (!uri?.startsWith("zcash:")) return;
    try {
      await navigator.clipboard.writeText(uri);
      setStatus("Payment link copied. Check the recipient and amount in your testnet wallet before approval.");
    } catch {
      setStatus("Copy failed. Try opening this payment in your wallet.");
    }
  }

  async function payWithConnectedWallet(allowTransparent = false) {
    if (!summary || !record || record.state !== "prepared" || busy) return;
    const wallet = connection.status === "connected" ? connection.selected : null;
    if (!wallet) {
      setStatus("Choose a compatible payment wallet first.");
      return;
    }
    setBusy(true);
    try {
      const action = paymentAction(wallet, summary, { allowTransparent });
      if (action.kind !== "direct") throw new Error("Open the complete payment request in your wallet instead.");
      setStatus(`Review and approve the ${action.mode} payment in your wallet…`);
      const send = action.mode === "shielded" ? wallet.sendShieldedPayment! : wallet.sendTransparentPayment!;
      const txid = await send(action.payment);
      await submitTxid(record.id, txid);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "The wallet did not complete the payment.");
    } finally {
      setBusy(false);
    }
  }

  async function submitTxid(id: string, txid: string) {
    const response = await fetch("/api/zerant/zcash/payments/" + encodeURIComponent(id) + "/submit", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ txid: txid.trim() }),
    });
    if (!response.ok) {
      throw new Error("Zerant could not save the wallet transaction ID. Keep this ID and retry below: " + txid);
    }
    const saved = (await response.json()) as PaymentRecord;
    setRecord((current) => current?.id === id ? saved : current);
    setTxidDrafts((current) => ({ ...current, [id]: "" }));
    await refreshRecords().catch(() => undefined);
    setStatus("Transaction submitted. Zerant saved it. Network verification is pending.");
  }

  async function selectWallet(connector: ZcashConnector) {
    if (!summary) return;
    if (connector.capabilities.has("zip321Handoff")) {
      const action = paymentAction(connector, summary);
      if (action.kind === "handoff") window.location.assign(action.uri);
      return;
    }
    try {
      const chain = await ensureZcashConfig();
      await connectConnector(connector, chain);
      setStatus("Wallet connected. Review the payment and approve only when you are ready.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Wallet connection failed.");
    }
  }

  async function requestSummary(path: string, body: object, success: string) {
    setBusy(true);
    setSummary(null);
    setRecord(null);
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
      if (canTrack(reviewed)) {
        const prepared = await fetch("/api/zerant/zcash/payments", {
          method: "POST",
          credentials: "same-origin",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ canonical_uri: reviewed.canonical_uri }),
        });
        if (!prepared.ok) throw new Error("Zerant could not save this payment. Try preparing it again before opening your wallet.");
        setRecord((await prepared.json()) as PaymentRecord);
        await refreshRecords().catch(() => undefined);
      }
      setSummary(reviewed);
      setUri(reviewed.canonical_uri);
      setStatus(canTrack(reviewed) ? success + " Zerant saved this payment." : "Request validated for review. Only one exact-amount payment can be tracked in Zerant.");
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
        <input id="zcash-payment-recipient" value={recipient} onChange={(event) => { setRecipient(event.target.value); setSummary(null); setRecord(null); }} disabled={!enabled || busy} spellCheck={false} autoComplete="off" placeholder="Zcash address" />
        <label htmlFor="zcash-payment-amount">Amount</label>
        <div className="zcash-amount-field">
          <input id="zcash-payment-amount" value={amount} onChange={(event) => { setAmount(event.target.value); setSummary(null); setRecord(null); }} disabled={!enabled || busy} inputMode="decimal" autoComplete="off" placeholder="0.00" />
          <span>ZEC</span>
        </div>
        <div className="vault-actions"><Button disabled={!enabled || busy || !recipient.trim() || !amount.trim()} onClick={() => void createPayment()}>{busy ? "Preparing…" : "Review payment"}</Button></div>
      </> : <>
        <label htmlFor="zcash-payment-uri">Zcash payment request</label>
        <textarea id="zcash-payment-uri" value={uri} onChange={(event) => { setUri(event.target.value); setSummary(null); setRecord(null); }} disabled={!enabled || busy} spellCheck={false} rows={5} placeholder="zcash:..." />
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

      {record?.state !== "submitted" ? <div className="payment-request-actions">
        {mode === "shielded" && record ? <Button disabled={busy} onClick={() => void payWithConnectedWallet()}>Approve shielded payment</Button> : null}
        {mode === "transparent" && record ? <Button disabled={busy} onClick={() => void payWithConnectedWallet(true)}>Approve transparent payment</Button> : null}
        {!mode && record ? <ZcashWalletSelector purpose="payment" triggerLabel="Choose payment wallet" onSelect={(connector) => void selectWallet(connector)} /> : null}
        <Button variant="secondary" disabled={busy} onClick={openInWalletApp}>Open in another wallet</Button>
        <Button variant="secondary" disabled={busy} onClick={() => void copyPaymentLink()}>Copy payment link</Button>
      </div> : null}
      {record?.state !== "submitted" ? <div className="payment-link-copy">
        <label htmlFor="reviewed-payment-link">Reviewed payment link</label>
        <textarea id="reviewed-payment-link" readOnly value={summary.canonical_uri} rows={3} />
        <p className="small muted">Use a wallet on the same Zcash network. Zingo PC documents testnet and payment-link support; other wallets may also support this format. Review the exact details again in your wallet.</p>
      </div> : null}
      {record ? <p className="small muted">Saved payment: {record.state === "submitted" ? "Submitted" : "Prepared"}. If your wallet opens separately, enter its transaction ID in Recent payments after submission.</p> : null}
      <p className="small muted payment-request-note">A wallet transaction ID records submission here. Zerant cannot verify testnet settlement yet, so this payment will remain pending.</p>
      {flow === "review" && summary.payments.length > 1 ? <div className="request-payment-list">{summary.payments.map((item) => <article key={item.index} className="request-payment"><div><span className="eyebrow">Recipient {item.index + 1}</span><span className="mono">{item.recipient}</span></div><dl><div><dt>Amount</dt><dd>{formatZec(item.amount_zat)}</dd></div><div><dt>Label</dt><dd>{item.label ?? "None"}</dd></div><div><dt>Message</dt><dd>{item.message ?? "None"}</dd></div><div><dt>Memo</dt><dd>{item.memo_present ? "Present" : "None"}</dd></div></dl></article>)}</div> : null}
    </div> : null}
    {enabled ? <div className="request-summary" aria-label="Recent Zcash payments">
      <div className="section-heading compact"><p className="eyebrow">Recent payments</p><h3>Saved payment activity</h3></div>
      {records.length ? records.map((item) => <article className="request-payment" key={item.id}>
        <div><strong>{item.state === "submitted" ? "Submitted · Network verification pending" : item.state === "expired" ? "Expired" : "Prepared"}</strong><span className="mono">{item.recipient}</span></div>
        <p className="small muted">{formatZec(item.amount_zat)} · {new Date(item.created_at).toLocaleString()}</p>
        {item.txid ? <p className="small muted">Transaction <span className="mono">{item.txid}</span></p> : null}
        {item.state === "prepared" && item.payment_uri && item.transparent_only ? <p className="small muted">Transparent destination: this payment reveals more information on chain. Choose it only if that is acceptable to you.</p> : null}
        {item.state === "prepared" && item.payment_uri ? <div className="vault-actions wrap">
          <Button variant="secondary" onClick={() => reopenSavedPayment(item.payment_uri)}>{item.transparent_only ? "Open transparent payment in wallet" : "Open in wallet"}</Button>
          <Button variant="secondary" onClick={() => void copySavedPayment(item.payment_uri)}>{item.transparent_only ? "Copy transparent payment link" : "Copy payment link"}</Button>
        </div> : null}
        {item.state === "prepared" ? <div className="vault-actions wrap">
          <label htmlFor={"payment-txid-" + item.id}>Wallet transaction ID</label>
          <input id={"payment-txid-" + item.id} value={txidDrafts[item.id] ?? ""} onChange={(event) => setTxidDrafts((current) => ({ ...current, [item.id]: event.target.value }))} spellCheck={false} autoComplete="off" maxLength={64} placeholder="64-character transaction ID" />
          <Button variant="secondary" disabled={!/^[0-9a-f]{64}$/i.test(txidDrafts[item.id] ?? "")} onClick={() => void submitTxid(item.id, txidDrafts[item.id] ?? "").catch((error: unknown) => setStatus(error instanceof Error ? error.message : "Could not save transaction ID."))}>Save submission</Button>
        </div> : null}
      </article>) : <p className="muted">No saved payments yet.</p>}
      {nextCursor ? <div className="vault-actions"><Button variant="secondary" onClick={() => void loadMore()}>Load more payments</Button></div> : null}
    </div> : null}
  </section>;
}

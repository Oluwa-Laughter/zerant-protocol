"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { getInjectedZcashWallet, zatoshiToZec } from "@/lib/zcash-wallet";

type Payment = {
  index: number;
  recipient: string;
  amount_zat: number | null;
  memo_present: boolean;
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

function formatZec(zat: number | null): string {
  if (zat === null) return "Amount chosen by sender";
  return (zat / 100_000_000).toLocaleString(undefined, {
    maximumFractionDigits: 8,
  }) + " ZEC";
}

export function ZcashPaymentRequestReview({ enabled }: { enabled: boolean }) {
  const [uri, setUri] = useState("");
  const [summary, setSummary] = useState<Summary | null>(null);
  const [status, setStatus] = useState(
    enabled
      ? "Paste a real ZIP-321 payment request to review it."
      : "Connect your Zcash identity before reviewing payment requests.",
  );

  function openInWalletApp() {
    if (!summary) return;
    setStatus("Opening the validated payment request in your Zcash wallet…");
    window.location.href = summary.canonical_uri;
  }

  async function payWithBrowserWallet() {
    if (!summary || summary.payment_count !== 1) {
      setStatus("Use a wallet app for multi-recipient payment requests.");
      return;
    }
    const payment = summary.payments[0];
    if (
      !payment ||
      payment.amount_zat === null ||
      payment.memo_present ||
      payment.other_param_names.length > 0
    ) {
      setStatus(
        "This request needs wallet-app handling so all payment details remain intact.",
      );
      return;
    }

    try {
      const wallet = getInjectedZcashWallet();
      if (!wallet) {
        setStatus("No compatible Zcash browser wallet was detected.");
        return;
      }
      const existing = await wallet.existingConnection();
      if (!existing) {
        setStatus("Approve the browser wallet connection to continue…");
        await wallet.connect();
      }

      setStatus("Review and approve the shielded payment in your wallet…");
      const txid = await wallet.sendShieldedPayment({
        to: payment.recipient,
        amount: zatoshiToZec(payment.amount_zat),
      });
      const shortTxid = txid.length > 18 ? txid.slice(0, 10) + "…" + txid.slice(-6) : txid;
      setStatus("Payment submitted by your wallet. Transaction " + shortTxid + ".");
    } catch (error) {
      setStatus(
        error instanceof Error
          ? error.message
          : "The wallet did not complete the payment.",
      );
    }
  }

  async function inspect() {
    setSummary(null);
    const response = await fetch("/api/zerant/zcash/payment-request", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ uri }),
    });
    if (!response.ok) {
      if (response.status === 429) { setStatus("You’re doing that too quickly. Try again in a minute."); return; }
      setStatus(
        response.status === 401
          ? "Your Zerant session is not authenticated."
          : "That payment request is invalid or unsupported.",
      );
      return;
    }
    const result = (await response.json()) as Summary;
    setSummary(result);
    setStatus("Payment request validated by the native Zcash parser.");
  }

  return (
    <section className="zcash-request-review" aria-labelledby="zcash-request-title">
      <div className="section-heading">
        <p className="eyebrow">Zcash payment request</p>
        <h2 id="zcash-request-title">Review before wallet approval.</h2>
        <p className="muted">
          Zerant checks the payment request and shows exactly who will receive funds, how much is requested, and what information accompanies the request before you approve anything.
        </p>
      </div>

      <div className="request-review-panel">
        <label htmlFor="zcash-payment-uri">Zcash payment request URI</label>
        <textarea
          id="zcash-payment-uri"
          value={uri}
          onChange={(event) => setUri(event.target.value)}
          disabled={!enabled}
          spellCheck={false}
          rows={5}
          placeholder="zcash:..."
        />
        <div className="vault-actions">
          <Button disabled={!enabled || !uri.trim()} onClick={inspect}>
            Validate request
          </Button>
        </div>
        <p className="vault-status neutral" role="status">{status}</p>
      </div>

      {summary ? (
        <div className="request-summary">
          <div className="request-summary-top">
            <div>
              <span className="eyebrow">Canonical request</span>
              <span className="mono">{summary.canonical_uri}</span>
            </div>
            <div>
              <span className="eyebrow">Total</span>
              <strong>{formatZec(summary.total_zat)}</strong>
            </div>
          </div>

          <div className="payment-request-actions">
            <Button onClick={openInWalletApp}>Open in Zcash wallet</Button>
            <Button
              variant="secondary"
              onClick={payWithBrowserWallet}
              disabled={
                summary.payment_count !== 1 ||
                summary.payments[0]?.amount_zat === null ||
                summary.payments[0]?.memo_present ||
                Boolean(summary.payments[0]?.other_param_names.length)
              }
            >
              Pay from shielded browser wallet
            </Button>
          </div>
          {summary.payment_count !== 1 ||
          summary.payments[0]?.amount_zat === null ||
          summary.payments[0]?.memo_present ||
          Boolean(summary.payments[0]?.other_param_names.length) ? (
            <p className="small muted payment-request-note">
              This request should be opened in a wallet app so every ZIP-321 field is preserved.
            </p>
          ) : (
            <p className="small muted payment-request-note">
              Browser-wallet payment uses shielded funds by default and still requires approval in
              your wallet.
            </p>
          )}

          <div className="request-payment-list">
            {summary.payments.map((payment) => (
              <article key={payment.index} className="request-payment">
                <div>
                  <span className="eyebrow">Recipient {payment.index + 1}</span>
                  <span className="mono">{payment.recipient}</span>
                </div>
                <dl>
                  <div><dt>Amount</dt><dd>{formatZec(payment.amount_zat)}</dd></div>
                  <div><dt>Label</dt><dd>{payment.label ?? "None"}</dd></div>
                  <div><dt>Message</dt><dd>{payment.message ?? "None"}</dd></div>
                  <div><dt>Memo</dt><dd>{payment.memo_present ? "Present" : "None"}</dd></div>
                </dl>
              </article>
            ))}
          </div>
        </div>
      ) : null}
    </section>
  );
}

"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";

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

  async function inspect() {
    setSummary(null);
    const response = await fetch("/api/zerant/zcash/payment-request", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ uri }),
    });
    if (!response.ok) {
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
          Zerant validates ZIP-321 on the Rust server and shows the request in a bounded form.
          It does not construct, prove, or sign the transaction in browser JavaScript.
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

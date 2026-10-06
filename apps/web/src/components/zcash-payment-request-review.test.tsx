import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { checkPaymentRecipient, manualPaymentDetails, paymentNetworkStatus, shouldAutoObservePayment, shouldObserveAfterSubmit, ZcashPaymentRequestReview } from "./zcash-payment-request-review";

test("payment preparation requires a validated testnet destination without wallet connection", async () => {
  const requests: string[] = [];
  const inspect = ((input: string) => {
    requests.push(input);
    return Promise.resolve(Response.json({ network: "testnet" }));
  }) as typeof fetch;
  await checkPaymentRecipient("utest1recipient", inspect);
  assert.deepEqual(requests, ["/api/zerant/zcash/address"]);
  await assert.rejects(checkPaymentRecipient("zr_example", inspect), /not a payment address/);
  await assert.rejects(checkPaymentRecipient("zcash:utest1example?amount=1", inspect), /Review a payment request/);
  assert.equal(requests.length, 1);
  await assert.rejects(checkPaymentRecipient("u1mainnet", (() => Promise.resolve(Response.json({ network: "mainnet" }))) as typeof fetch), /not on Zcash testnet/);
  await assert.rejects(checkPaymentRecipient("invalid", (() => Promise.resolve(Response.json({ error: "invalid request" }, { status: 400 }))) as typeof fetch), /not a valid Zcash payment address/);
});

test("manual handoff keeps only one exact simple payment", () => {
  const payment = { index: 0, recipient: "utest1example", amount_zat: 123456789,
    memo_present: false, transparent_only: false, can_receive_memo: true,
    label: null, message: null, other_param_names: [] as string[] };
  const summary = { canonical_uri: "zcash:utest1example?amount=1.23456789", payment_count: 1,
    total_zat: 123456789, payments: [payment] };
  assert.deepEqual(manualPaymentDetails(summary), { recipient: payment.recipient, amountZec: "1.23456789" });
  assert.equal(manualPaymentDetails({ ...summary, payment_count: 2, payments: [payment, payment] }), null);
  assert.equal(manualPaymentDetails({ ...summary, payments: [{ ...payment, memo_present: true }] }), null);
  assert.equal(manualPaymentDetails({ ...summary, payments: [{ ...payment, message: "Keep this exact" }] }), null);
  assert.equal(manualPaymentDetails({ ...summary, payments: [{ ...payment, amount_zat: Number.MAX_SAFE_INTEGER + 1 }] }), null);
});

test("Zcash payment experience starts with sender-side private payment review", () => {
  const html = renderToStaticMarkup(<ZcashPaymentRequestReview enabled />);
  assert.ok(html.includes("Send ZEC"));
  assert.ok(html.includes("Private payment"));
  assert.ok(html.includes("Recipient’s Zcash testnet address"));
  assert.ok(html.includes("Your wallet remains in control"));
  assert.equal(html.includes("Choose direct wallet"), false);
  assert.ok(html.includes("Review payment"));
  assert.ok(html.includes("Saved payment activity"));
  assert.ok(html.includes("No saved payments yet"));
  assert.equal(html.includes("wallet balance"), false);
  assert.equal(html.includes("transaction history"), false);
});


test("payment network labels never overstate shielded intent verification", () => {
  assert.equal(paymentNetworkStatus({ state: "submitted", network_state: null, confirmations: null, min_confirmations: 10 }).label, "Submitted · pending");
  assert.equal(paymentNetworkStatus({ state: "submitted", network_state: "mempool", confirmations: 0, min_confirmations: 10 }).label, "Seen in mempool");
  assert.equal(paymentNetworkStatus({ state: "submitted", network_state: "mined", confirmations: 3, min_confirmations: 10 }).label, "Mined · 3/10");
  assert.equal(paymentNetworkStatus({ state: "submitted", network_state: "mined", confirmations: 10, min_confirmations: 10 }).label, "Depth reached · 10/10");
  assert.notEqual(paymentNetworkStatus({ state: "submitted", network_state: "mined", confirmations: 10, min_confirmations: 10 }).label, "Payment verified");
});


test("auto observation is bounded to stale submitted payments below target depth", () => {
  const now = Date.parse("2026-10-05T20:00:00Z");
  assert.equal(shouldAutoObservePayment({ state: "prepared", network_state: null, confirmations: null, min_confirmations: 10, observed_at: null }, now), false);
  assert.equal(shouldAutoObservePayment({ state: "submitted", network_state: null, confirmations: null, min_confirmations: 10, observed_at: null }, now), true);
  assert.equal(shouldAutoObservePayment({ state: "submitted", network_state: "mempool", confirmations: 0, min_confirmations: 10, observed_at: "2026-10-05T19:59:45Z" }, now), false);
  assert.equal(shouldAutoObservePayment({ state: "submitted", network_state: "mempool", confirmations: 0, min_confirmations: 10, observed_at: "2026-10-05T19:59:00Z" }, now), true);
  assert.equal(shouldAutoObservePayment({ state: "submitted", network_state: "mined", confirmations: 10, min_confirmations: 10, observed_at: "2026-10-05T19:00:00Z" }, now), false);
});


test("immediate observation only runs for newly submitted unobserved payments", () => {
  assert.equal(shouldObserveAfterSubmit(true, { state: "submitted", network_state: null }), true);
  assert.equal(shouldObserveAfterSubmit(false, { state: "submitted", network_state: null }), false);
  assert.equal(shouldObserveAfterSubmit(true, { state: "prepared", network_state: null }), false);
  assert.equal(shouldObserveAfterSubmit(true, { state: "submitted", network_state: "mempool" }), false);
});

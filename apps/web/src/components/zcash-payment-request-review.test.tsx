import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { paymentNetworkStatus, ZcashPaymentRequestReview } from "./zcash-payment-request-review";

test("Zcash payment experience starts with sender-side private payment review", () => {
  const html = renderToStaticMarkup(<ZcashPaymentRequestReview enabled />);
  assert.ok(html.includes("Send ZEC"));
  assert.ok(html.includes("Private payment"));
  assert.ok(html.includes("Review payment"));
  assert.ok(html.includes("Your wallet remains in control"));
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

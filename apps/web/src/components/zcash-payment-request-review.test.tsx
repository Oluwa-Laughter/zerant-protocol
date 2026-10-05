import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { ZcashPaymentRequestReview } from "./zcash-payment-request-review";

test("Zcash payment experience starts with sender-side private payment review", () => {
  const html = renderToStaticMarkup(<ZcashPaymentRequestReview enabled />);
  assert.ok(html.includes("Send ZEC"));
  assert.ok(html.includes("Private payment"));
  assert.ok(html.includes("Review payment"));
  assert.ok(html.includes("Your wallet remains in control"));
  assert.equal(html.includes("wallet balance"), false);
  assert.equal(html.includes("transaction history"), false);
});

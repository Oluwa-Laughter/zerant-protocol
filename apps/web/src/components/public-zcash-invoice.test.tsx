import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { PublicZcashInvoice } from "./public-zcash-invoice";

const openInvoice = {
  id: "11111111-1111-4111-8111-111111111111",
  amount_zat: 125_000_000,
  network: "zcash:testnet",
  state: "open" as const,
  expires_at: "2026-10-07T12:00:00Z",
  payment_uri: "zcash:tmExample?amount=1.25",
  transparent_only: false,
};

test("public invoice exposes only the payment request and honest lifecycle copy", () => {
  const html = renderToStaticMarkup(<PublicZcashInvoice invoice={openInvoice} />);
  assert.ok(html.includes("1.25 ZEC"));
  assert.ok(html.includes("Open payment request"));
  assert.ok(html.includes("Open in wallet"));
  assert.ok(html.includes("Refresh invoice status"));
  assert.ok(html.includes("payment request, not a payment receipt"));
  assert.equal(html.includes("zr_"), false);
  assert.equal(html.includes("wallet balance"), false);
  assert.equal(html.includes("transaction history"), false);
  assert.equal(html.includes("Payment verified"), false);
});

test("cancelled public invoice no longer exposes an active wallet action", () => {
  const html = renderToStaticMarkup(<PublicZcashInvoice invoice={{ ...openInvoice, state: "cancelled", payment_uri: null }} />);
  assert.ok(html.includes("Invoice cancelled"));
  assert.ok(html.includes("no longer exposes an active Zcash payment request"));
  assert.equal(html.includes("Open in wallet"), false);
  assert.equal(html.includes("Copy payment request"), false);
});

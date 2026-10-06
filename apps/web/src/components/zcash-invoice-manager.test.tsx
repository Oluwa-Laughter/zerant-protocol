import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { ZcashInvoiceManager } from "./zcash-invoice-manager";

test("invoice manager creates a request surface without claiming settlement", () => {
  const html = renderToStaticMarkup(<ZcashInvoiceManager enabled={true} />);
  assert.ok(html.includes("Request ZEC"));
  assert.ok(html.includes("Create a shareable Zcash invoice."));
  assert.ok(html.includes("payment request, not proof that you were paid"));
  assert.ok(html.includes("Invoices expire after 24 hours"));
  assert.ok(html.includes("Create invoice"));
  assert.equal(html.includes("Payment verified"), false);
  assert.equal(html.includes("wallet balance"), false);
  assert.equal(html.includes("transaction history"), false);
});

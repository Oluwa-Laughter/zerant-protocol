import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { ProductWorkspace } from "./product-workspace";

test("workspace renders server and Zcash boundaries without browser-local persistence", () => {
  const html = renderToStaticMarkup(
    <ProductWorkspace session={null} zcash={null} backendAvailable={false} />,
  );

  for (const text of [
    "Connect. Receive. Prove.",
    "Connect your Zcash identity",
    "PCZT + FROST boundary.",
    "API not configured.",
  ]) {
    assert.ok(html.includes(text), "missing production state: " + text);
  }

  for (const forbidden of [
    "grants.example",
    "payment.invoice_paid",
    "100000",
    "issuer:identity-demo",
    "IndexedDB",
    "Web Crypto",
    "localStorage",
  ]) {
    assert.equal(html.includes(forbidden), false, "browser-local or seeded data leaked: " + forbidden);
  }
});

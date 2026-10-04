import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { ProductWorkspace } from "./product-workspace";

test("product workspace renders empty real-integration states without seeded product data", () => {
  const html = renderToStaticMarkup(<ProductWorkspace />);

  for (const text of [
    "No request waiting for you.",
    "No issuer connected.",
    "No verification session active.",
    "No native wallet connection.",
    "Native verification is ready.",
    "Receive. Review. Prove.",
  ]) {
    assert.ok(html.includes(text), `missing production empty state: ${text}`);
  }

  for (const forbidden of [
    "grants.example",
    "payment.invoice_paid",
    "100000",
    "issuer:identity-demo",
    "synthetic payment",
    "illustrative origin",
  ]) {
    assert.equal(html.includes(forbidden), false, `seeded product data leaked: ${forbidden}`);
  }
});

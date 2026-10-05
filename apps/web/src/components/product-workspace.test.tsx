import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { ProductWorkspace } from "./product-workspace";

test("workspace renders server and Zcash boundaries without browser-local persistence", () => {
  const html = renderToStaticMarkup(
    <ProductWorkspace session={null} zcash={null} zcashNetwork={null} />,
  );

  for (const text of [
    "Connect. Receive. Prove.",
    "Open your Zerant account",
    "Require more than one person when it matters.",
    "Issue trust. Verify privately.",
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
  assert.equal(html.includes('id="zcash-wallet-actions"'), false);
});

test("signed-in workspace offers wallet actions beside payment review", () => {
  const html = renderToStaticMarkup(
    <ProductWorkspace
      session={{ authenticated: true, identity: "test-identity", scopes: [] }}
      zcash={null}
      zcashNetwork={null}
    />,
  );

  assert.ok(html.includes('id="zcash-wallet-actions"'));
  assert.ok(html.includes("Connect a Zcash wallet"));
  assert.ok(html.includes('id="zcash-payment-review"'));
  assert.ok(html.indexOf('id="zcash-wallet-actions"') < html.indexOf('id="zcash-payment-review"'));
});

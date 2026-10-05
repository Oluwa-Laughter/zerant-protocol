import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { ProductWorkspace } from "./product-workspace";

test("workspace renders server and Zcash boundaries without browser-local persistence", () => {
  const html = renderToStaticMarkup(
    <ProductWorkspace session={null} zcash={null} zcashNetwork={null} />,
  );

  for (const text of [
    "Private trust for the Zcash ecosystem.",
    "Sign in. Receive. Prove. Pay when needed.",
    "Open your Zerant account",
    "Use Zcash testnet for payments",
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

test("signed-in workspace links into the dedicated Zcash workspace", () => {
  const html = renderToStaticMarkup(
    <ProductWorkspace
      session={{ authenticated: true, identity: "test-identity", scopes: [] }}
      zcash={null}
      zcashNetwork={null}
      attention={{ activeCredentials: 3, pendingRequests: 2, preparedPayments: 1, submittedPayments: 1 }}
    />,
  );

  assert.ok(html.includes('href="/zcash"'));
  assert.ok(html.includes("Prepare a Zcash payment"));
  assert.ok(html.includes("Your Zerant activity at a glance."));
  assert.ok(html.includes("Waiting for your decision"));
  assert.ok(html.includes("Settlement verification pending"));
  assert.equal(html.includes('id="zcash-wallet-actions"'), false);
  assert.equal(html.includes('id="zcash-payment-review"'), false);
});


test("workspace renders degraded Zcash state as protected mode", () => {
  const html = renderToStaticMarkup(
    <ProductWorkspace
      session={{ authenticated: true, identity: "test-identity", scopes: [] }}
      zcash={null}
      zcashNetwork={{
        configured: true,
        network: "testnet",
        state: "degraded",
        network_actions_enabled: false,
        synced: false,
        block_height: 123,
        estimated_height: 125,
        lag: 2,
        last_confirmed_at: "2026-10-05T00:00:00Z",
      }}
    />,
  );

  assert.ok(html.includes("Zcash network temporarily unavailable."));
  assert.ok(html.includes("Protected mode"));
  assert.ok(html.includes("protecting network-dependent actions"));
  assert.equal(html.includes("PostgreSQL"), false);
  assert.equal(html.includes("lightwalletd"), false);
  assert.equal(html.includes("Zaino"), false);
});

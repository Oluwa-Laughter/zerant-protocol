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
      attention={{ activeCredentials: 3, pendingRequests: 2, preparedPayments: 1, submittedPayments: 3, unseenSubmittedPayments: 1, networkSeenPayments: 2, depthReachedPayments: 1, forkedPayments: 0, openInvoices: 2 }}
    />,
  );

  assert.ok(html.includes('href="/zcash"'));
  assert.ok(html.includes("Prepare a Zcash payment"));
  assert.ok(html.includes("Your Zerant activity at a glance."));
  assert.ok(html.includes("Waiting for your decision"));
  assert.ok(html.includes("2 seen on network · 1 depth reached · 1 unseen"));
  assert.ok(html.includes("2</span><strong>open invoices"));
  assert.ok(html.includes('href="/zcash#zcash-invoices"'));
  assert.equal(html.includes('id="zcash-wallet-actions"'), false);
  assert.equal(html.includes('id="zcash-payment-review"'), false);
});


test("workspace renders degraded Zcash state as protected mode", () => {
  const html = renderToStaticMarkup(
    <ProductWorkspace
      session={{ authenticated: true, identity: "test-identity", scopes: [] }}
      zcash={{ capabilities: ["read"], receipt_verification: false, sendmany_advertised: false, sendfromaccount_advertised: false, pczt_complete: false, wallet: "native", chain_height: 123 }}
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
  assert.ok(html.includes('class="workspace-state"><span></span>Protected mode'));
  assert.equal(html.includes('class="workspace-state ready"><span></span>Protected mode'), false);
  assert.equal(html.includes("PostgreSQL"), false);
  assert.equal(html.includes("lightwalletd"), false);
  assert.equal(html.includes("Zaino"), false);
});


test("workspace surfaces forked Zcash submissions as attention", () => {
  const html = renderToStaticMarkup(
    <ProductWorkspace
      session={{ authenticated: true, identity: "test-identity", scopes: [] }}
      zcash={null}
      zcashNetwork={null}
      attention={{ activeCredentials: 0, pendingRequests: 0, preparedPayments: 0, submittedPayments: 1, unseenSubmittedPayments: 0, networkSeenPayments: 0, depthReachedPayments: 0, forkedPayments: 1, openInvoices: 0 }}
    />,
  );
  assert.ok(html.includes("1 transaction no longer on the best chain"));
  assert.equal(html.includes("Payment verified"), false);
});

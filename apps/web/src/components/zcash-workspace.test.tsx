import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { ZcashWorkspace } from "./zcash-workspace";

const session = { authenticated: true, identity: "zr_test", scopes: ["auth"] };
const network = {
  configured: true,
  network: "zcash:testnet",
  state: "ready" as const,
  network_actions_enabled: true,
  synced: true,
  block_height: 1,
  estimated_height: 1,
  lag: 0,
  last_confirmed_at: "2026-10-05T00:00:00Z",
};
const zcash = {
  capabilities: [],
  receipt_verification: false,
  sendmany_advertised: false,
  sendfromaccount_advertised: false,
  pczt_complete: false,
  wallet: "configured",
  chain_height: 1,
};

test("dedicated Zcash workspace owns wallet and payment actions", () => {
  const html = renderToStaticMarkup(<ZcashWorkspace session={session} zcash={zcash} network={network} />);
  assert.ok(html.includes("Zerant on Zcash · Testnet"));
  assert.ok(html.includes("Your Zerant ID is not a Zcash address."));
  assert.ok(html.includes("Testnet wallet setup"));
  assert.ok(html.includes("Open official Noir releases"));
  assert.ok(html.includes("noir-wallet-sdk/releases"));
  assert.ok(html.includes("[Testnet] Noir Wallet"));
  assert.ok(html.includes("ZIP-321 testnet wallet"));
  assert.ok(html.includes("github.com/zingolabs/zingo-pc"));
  assert.ok(html.includes("Never enter a recovery phrase"));
  assert.ok(html.includes('id="zcash-wallet-actions"'));
  assert.ok(html.includes('href="#zcash-payment-review"'));
  assert.ok(html.includes('href="#zcash-address-inspector"'));
  assert.ok(html.includes('href="#zcash-wallet-actions"'));
  assert.ok(html.includes("Prepare, review, then approve in your wallet."));
  assert.ok(html.includes("Testnet settlement verification is not available yet"));
  assert.ok(html.includes("Prepared"));
  assert.ok(html.includes("Submitted · pending"));
  assert.equal(html.includes(">Confirmed<"), false);
  assert.ok(html.includes("Pasting a wallet address does not connect a wallet"));
});

test("signed-out Zcash workspace directs users to account access", () => {
  const html = renderToStaticMarkup(<ZcashWorkspace session={null} zcash={null} network={null} />);
  assert.ok(html.includes("Sign in before using private Zcash actions."));
  assert.ok(html.includes("Choose a sign-in method"));
  assert.equal(html.includes('id="zcash-wallet-actions"'), false);
  assert.equal(html.includes('href="#zcash-wallet-actions"'), false);
});

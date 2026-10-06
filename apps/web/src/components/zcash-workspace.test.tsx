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
  assert.ok(html.includes("Live workflow"));
  assert.ok(html.includes("From Zerant account to Zcash payment."));
  assert.ok(html.includes("Refresh network"));
  assert.ok(html.includes("Track submission"));

  assert.ok(html.includes("Testnet wallet setup"));
  assert.ok(html.includes("Bring a Zcash testnet wallet."));
  assert.ok(html.includes("Open, scan, or copy"));
  assert.ok(html.includes("Get the recipient’s testnet address"));
  assert.equal(html.includes('id="zcash-wallet-actions"'), false);
  assert.ok(html.includes('href="#zcash-payment-review"'));
  assert.ok(html.includes('href="#zcash-address-inspector"'));
  assert.equal(html.includes('href="#zcash-wallet-actions"'), false);
  assert.ok(html.includes("Prepare, review, then approve in your wallet."));
  assert.ok(html.includes("Network visibility does not independently verify a shielded recipient or amount"));
  assert.ok(html.includes("Prepared"));
  assert.ok(html.includes("Submitted"));
  assert.ok(html.includes("Seen"));
  assert.ok(html.includes("Mined · depth"));
  assert.equal(html.includes("Testnet settlement verification is not available yet"), false);
  assert.equal(html.includes(">Payment verified<"), false);
  assert.ok(html.includes("Payments need a Zcash testnet receive address"));
});

test("signed-out Zcash workspace directs users to account access", () => {
  const html = renderToStaticMarkup(<ZcashWorkspace session={null} zcash={null} network={null} />);
  assert.ok(html.includes("Sign in before using private Zcash actions."));
  assert.ok(html.includes("Choose a sign-in method"));
  assert.equal(html.includes('id="zcash-wallet-actions"'), false);
  assert.equal(html.includes('href="#zcash-wallet-actions"'), false);
});


test("degraded Zcash workspace never renders the network state as ready", () => {
  const degraded = { ...network, state: "degraded" as const, network_actions_enabled: false, synced: false };
  const html = renderToStaticMarkup(<ZcashWorkspace session={session} zcash={zcash} network={degraded} />);
  assert.ok(html.includes("Zcash network temporarily unavailable."));
  assert.ok(html.includes("Protected mode"));
  assert.equal(html.includes('workspace-state ready'), false);
});

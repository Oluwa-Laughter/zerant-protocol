import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { ZcashPayoutDestinations } from "./zcash-payout-destinations";

test("private payout sharing keeps payment destination separate from identity", () => {
  const html = renderToStaticMarkup(<ZcashPayoutDestinations enabled={true} />);
  assert.ok(html.includes("Private payout details"));
  assert.ok(html.includes("Your Zerant ID stays your trust identity"));
  assert.ok(html.includes("Shielded-capable Zcash testnet address"));
  assert.ok(html.includes("Transparent-only destinations are rejected"));
  assert.ok(html.includes("expire after seven days"));
  assert.ok(html.includes("does not link the wallet to your Zerant identity"));
  assert.equal(html.includes("seed phrase"), true);
  assert.equal(html.includes("wallet history"), true);
  assert.equal(html.includes("Connect wallet"), false);
});

test("signed-out payout sharing remains account-gated", () => {
  const html = renderToStaticMarkup(<ZcashPayoutDestinations enabled={false} />);
  assert.ok(html.includes("Sign in to share private payout details."));
  assert.ok(html.includes("Share payout destination"));
});

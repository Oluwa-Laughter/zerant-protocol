import { test } from "node:test";
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import { ConsentPanel } from "./consent-panel";
import { scenarios } from "../lib/demo-data";

test("every scenario renders a narrow disclosure boundary", () => {
  for (const scenario of scenarios) {
    const html = renderToStaticMarkup(<ConsentPanel scenario={scenario} />);
    for (const text of [
      scenario.request.origin,
      scenario.request.claim,
      "Requested",
      "shared",
      "Kept private",
      "Audience subject public key",
      "Request ID, request digest, challenge, nonce and origin",
      "not authenticated",
      "not zero knowledge",
    ]) {
      assert.ok(html.includes(text), `${scenario.id}: missing consent boundary: ${text}`);
    }
    assert.ok(!html.includes("<input"), `${scenario.id}: consent must not hide preselected attributes`);
  }
});

test("payment scenario withholds wallet and transaction details", () => {
  const scenario = scenarios.find((item) => item.id === "payment");
  assert.ok(scenario);
  const html = renderToStaticMarkup(<ConsentPanel scenario={scenario} />);
  for (const text of ["Transaction ID", "Amount and memo", "Wallet addresses, balances and transaction history"]) {
    assert.ok(html.replaceAll("&amp;", "&").includes(text.replaceAll("&amp;", "&")), `Missing payment privacy boundary: ${text}`);
  }
});

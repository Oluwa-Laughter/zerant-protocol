import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import Home from "../app/page";

test("landing page explains Zerant's Zcash role without claiming testnet settlement", () => {
  const html = renderToStaticMarkup(<Home />);
  assert.ok(html.includes("Built for the Zcash ecosystem · Testnet"));
  assert.ok(html.includes("Zcash testnet handles payments"));
  assert.ok(html.includes("Zerant handles credentials, requests and consent"));
  assert.ok(html.includes("Submission is not settlement"));
  assert.ok(html.includes('href="/zcash"'));
  assert.equal(html.includes("Payment evidence is verified"), false);
  assert.equal(html.includes("Zcash-native"), false);
});

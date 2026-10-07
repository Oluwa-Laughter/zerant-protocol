import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { IssuerPayoutInbox } from "./issuer-payout-inbox";

test("organization payout inbox treats addresses as private purpose-scoped data", () => {
  const html = renderToStaticMarkup(<IssuerPayoutInbox enabled={true} />);
  assert.ok(html.includes("Private payout inbox"));
  assert.ok(html.includes("Address ≠ Zerant identity"));
  assert.ok(html.includes("Do not copy these destinations into CRM profiles or credential claims"));
  assert.ok(html.includes("Prepare directly"));
  assert.ok(html.includes("without putting the private destination in a URL"));
  assert.ok(html.includes("seven days"));
  assert.equal(html.includes("Holder Zerant ID"), false);
  assert.equal(html.includes("holder_zerant_id"), false);
  assert.equal(html.includes("Copy receive address"), false);
  assert.equal(html.includes("wallet balance"), false);
  assert.equal(html.includes("transaction history"), false);
});

test("auditor-like disabled access does not expose payout data", () => {
  const html = renderToStaticMarkup(<IssuerPayoutInbox enabled={false} />);
  assert.ok(html.includes("Your organization role cannot access private payout destinations."));
  assert.equal(html.includes("Copy receive address"), false);
});

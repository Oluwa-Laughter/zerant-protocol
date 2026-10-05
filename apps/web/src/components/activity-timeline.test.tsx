import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { ActivityTimeline } from "./activity-timeline";

test("activity timeline groups real account events without exposing payloads", () => {
  const html = renderToStaticMarkup(<ActivityTimeline initialPage={{
    items: [
      { id: 1, event_type: "credential_issued", object_id: "cred-1", label: "Membership", context: "community", counterparty: "Example issuer", created_at: "2026-10-05T12:00:00Z" },
      { id: 2, event_type: "verification_approved", object_id: "req-1", label: "Membership proof", context: "community", counterparty: "Verifier App", created_at: "2026-10-05T12:05:00Z" },
      { id: 3, event_type: "verifier_api_key_revoked", object_id: "key-1", label: "Production backend", context: null, counterparty: null, created_at: "2026-10-05T12:10:00Z" },
    ],
    next_cursor: null,
  }} />);
  assert.ok(html.includes("Recorded events"));
  assert.ok(html.includes("Credentials"));
  assert.ok(html.includes("Verification"));
  assert.ok(html.includes("Access &amp; security"));
  assert.ok(html.includes("Membership proof"));
  assert.equal(html.includes("wallet balance"), false);
  assert.equal(html.includes("credential.value"), false);
});

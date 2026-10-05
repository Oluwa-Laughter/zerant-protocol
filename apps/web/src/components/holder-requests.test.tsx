import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { HolderRequests } from "./holder-requests";

test("holder must review the claim before approval, while denial stays available", () => {
  const html = renderToStaticMarkup(
    <HolderRequests
      authenticated
      backendAvailable
      initialRequests={[{
        id: "00000000-0000-4000-8000-000000000001",
        verifier_name: "Requesting organization",
        verifier_origin: "https://verifier.example",
        purpose: "Check membership",
        credential_name: "Membership",
        claim_type: "membership",
        context: "organization",
        created_at: "2026-10-05T12:00:00Z",
        expires_at: "2026-10-06T12:00:00Z",
      }]}
    />,
  );

  assert.ok(html.includes("Review proof"));
  assert.ok(html.includes("Deny"));
  assert.ok(html.includes("verifier.example"));
  assert.equal(html.includes("Approve this claim"), false);
});

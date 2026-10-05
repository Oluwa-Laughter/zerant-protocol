import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { VerifierWorkspace } from "./verifier-workspace";

const issuer = {
  display_name: "Example Issuer",
  issuer_id: "issuer_example",
  schemas: [{
    id: "11111111-1111-4111-8111-111111111111",
    issuer_id: "issuer_example",
    issuer_name: "Example Issuer",
    display_name: "Contributor",
    description: "Confirms a contribution role.",
    claim_type: "role",
    context: "community",
    default_expiry_days: 30,
    version: 1,
    active: true,
    supersedes_schema_id: null,
    retired_at: null,
    created_at: "2026-10-05T00:00:00Z",
  }],
};

test("verifier requires review before a request can be sent", () => {
  const html = renderToStaticMarkup(<VerifierWorkspace
    authenticated={true}
    backendAvailable={true}
    initialProfile={{ display_name: "Verifier App", origin: "https://verifier.example", created_at: "2026-10-05T00:00:00Z" }}
    issuers={[issuer]}
    initialRequests={[]}
    initialKeys={[]}
    initialApiKeys={[]}
    initialWebhooks={[]}
  />);
  assert.ok(html.includes("What do you need to verify?"));
  assert.ok(html.includes("Review request"));
  assert.ok(html.includes("recipient has five minutes to review the exact claim"));
  assert.equal(html.includes("Send verification request</button>"), false);
});

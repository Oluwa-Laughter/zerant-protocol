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
    initialProfile={{ display_name: "Verifier App", origin: "https://verifier.example", retired_at: null, created_at: "2026-10-05T00:00:00Z" }}
    issuers={[issuer]}
    initialRequests={[]}
    initialKeys={[]}
    initialApiKeys={[]}
    initialWebhooks={[]}
  />);
  assert.ok(html.includes("What do you need to verify?"));
  assert.ok(html.includes("Review request"));
  assert.ok(html.includes("Refresh requests"));
  assert.ok(html.includes("Retire verifier profile"));
  assert.ok(html.includes("preserving verifier-side request, proof, and signing-key history"));
  assert.ok(html.includes("Ask for one fact. Receive one bounded result."));
  assert.ok(html.includes('href="#new-verification-request"'));
  assert.ok(html.includes('href="#verification-results"'));
  assert.ok(html.includes("1 credential type available"));
  assert.ok(html.includes("recipient has five minutes to review the exact claim"));
  assert.equal(html.includes("Send verification request</button>"), false);
});


test("verified request offers a bounded result action without raw proof material", () => {
  const html = renderToStaticMarkup(<VerifierWorkspace
    authenticated={true}
    backendAvailable={true}
    initialProfile={{ display_name: "Verifier App", origin: "https://verifier.example", retired_at: null, created_at: "2026-10-05T00:00:00Z" }}
    issuers={[issuer]}
    initialRequests={[{
      id: "11111111-1111-4111-8111-111111111112",
      holder_zerant_id: "zr_holder",
      purpose: "Check contributor status",
      credential_schema_id: issuer.schemas[0].id,
      credential_name: "Contributor",
      claim_type: "role",
      context: "community",
      status: "approved",
      verified: true,
      created_at: "2026-10-05T12:00:00Z",
      expires_at: "2026-10-05T12:05:00Z",
    }]}
    initialKeys={[]}
    initialApiKeys={[]}
    initialWebhooks={[]}
  />);
  assert.ok(html.includes("View narrow result"));
  assert.equal(html.includes("response_jws"), false);
  assert.equal(html.includes("revocation_jws"), false);
  assert.equal(html.includes("wallet address"), false);
});


test("retired verifier preserves history and closes new work", () => {
  const html = renderToStaticMarkup(<VerifierWorkspace
    authenticated={true}
    backendAvailable={true}
    initialProfile={{ display_name: "Verifier App", origin: "https://verifier.example", retired_at: "2026-10-06T09:00:00Z", created_at: "2026-10-05T00:00:00Z" }}
    issuers={[issuer]}
    initialRequests={[{
      id: "11111111-1111-4111-8111-111111111113", holder_zerant_id: "zr_holder", purpose: "Historical check",
      credential_schema_id: issuer.schemas[0].id, credential_name: "Contributor", claim_type: "role", context: "community",
      status: "approved", verified: true, created_at: "2026-10-05T12:00:00Z", expires_at: "2026-10-05T12:05:00Z",
    }]}
    initialKeys={[]}
    initialApiKeys={[]}
    initialWebhooks={[]}
  />);
  assert.ok(html.includes("Archive-only mode is active."));
  assert.ok(html.includes("Historical check"));
  assert.ok(html.includes("Historical verifier records remain available"));
  assert.equal(html.includes("This verifier profile has been removed."), false);
  assert.equal(html.includes("Retire verifier profile"), false);
});

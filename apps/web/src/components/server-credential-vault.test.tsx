import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { credentialLifecycleState, ServerCredentialVault } from "./server-credential-vault";

function credential(id: string, expired = false, revoked = false) {
  return {
    id, revoked, expired, created_at: "2026-10-05T12:00:00Z", updated_at: "2026-10-05T12:00:00Z",
    credential: {
      type: "zerant.private-credential", issuer: "issuer_example", credential_id: id, credential_name: "Membership",
      claim_type: "membership", value: "member", context: "community", issued_at: 1_700_000_000, expires_at: expired ? 1_700_000_001 : 4_102_444_800,
    },
  };
}

test("vault distinguishes active expired and revoked credentials", () => {
  const html = renderToStaticMarkup(<ServerCredentialVault
    initialSession={{ authenticated: true, identity: "identity", zerant_id: "zr_example", scopes: [] }}
    initialCredentials={[credential("active"), credential("expired", true), credential("revoked", false, true)]}
    backendAvailable={true}
  />);
  assert.ok(html.includes("Active"));
  assert.ok(html.includes("Expired"));
  assert.ok(html.includes("Revoked"));
  assert.ok(html.includes("This credential expired on"));
  assert.ok(html.includes("cannot be used for new proofs"));
  assert.ok(html.includes("Copy Zerant ID"));
});

test("credential lifecycle classifier prioritizes revocation then expiry", () => {
  assert.equal(credentialLifecycleState({ revoked: false, expired: false }), "active");
  assert.equal(credentialLifecycleState({ revoked: false, expired: true }), "expired");
  assert.equal(credentialLifecycleState({ revoked: true, expired: true }), "revoked");
});

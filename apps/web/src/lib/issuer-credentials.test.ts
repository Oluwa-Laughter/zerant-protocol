import assert from "node:assert/strict";
import test from "node:test";
import {
  credentialState,
  filterIssuedCredentials,
  type IssuedCredentialSummary,
} from "./issuer-credentials";

const now = Date.parse("2026-10-08T12:00:00Z");
const records: IssuedCredentialSummary[] = [
  { credential_schema_id: "member", holder_zerant_id: "zr_a", claim_type: "membership", context: "Community", issued_at: "2026-10-04T12:00:00Z", expires_at: "2026-12-08T12:00:00Z", revoked: false },
  { credential_schema_id: "member", holder_zerant_id: "zr_b", claim_type: "membership", context: "Community", issued_at: "2026-10-05T12:00:00Z", expires_at: "2026-10-07T12:00:00Z", revoked: false },
  { credential_schema_id: null, holder_zerant_id: "zr_c", claim_type: "contributor", context: "OSS", issued_at: "2026-10-06T12:00:00Z", expires_at: "2026-10-07T12:00:00Z", revoked: true },
];
const schemas = [{ id: "member", display_name: "Organization Membership" }];

test("credential state distinguishes active, expired and revoked", () => {
  assert.equal(credentialState(records[0], now), "active");
  assert.equal(credentialState(records[1], now), "expired");
  assert.equal(credentialState(records[2], now), "revoked");
  assert.equal(credentialState({ revoked: false, expires_at: "invalid" }, now), "expired");
  assert.equal(credentialState({ revoked: false, expires_at: "2026-10-08T12:00:00Z" }, now), "expired");
});

test("issuer history searches holders, credential names, claim types and contexts", () => {
  assert.equal(filterIssuedCredentials(records, schemas, "ZR_A", "all", now).length, 1);
  assert.equal(filterIssuedCredentials(records, schemas, "organization membership", "all", now).length, 2);
  assert.equal(filterIssuedCredentials(records, schemas, "oss", "all", now).length, 1);
  assert.equal(filterIssuedCredentials(records, schemas, "nonexistent", "all", now).length, 0);
});

test("issuer history filters lifecycle independently and sorts newest first", () => {
  assert.deepEqual(filterIssuedCredentials(records, schemas, "", "all", now).map((c) => c.holder_zerant_id), ["zr_c", "zr_b", "zr_a"]);
  assert.deepEqual(filterIssuedCredentials(records, schemas, "", "active", now).map((c) => c.holder_zerant_id), ["zr_a"]);
  assert.deepEqual(filterIssuedCredentials(records, schemas, "", "expired", now).map((c) => c.holder_zerant_id), ["zr_b"]);
  assert.deepEqual(filterIssuedCredentials(records, schemas, "", "revoked", now).map((c) => c.holder_zerant_id), ["zr_c"]);
});

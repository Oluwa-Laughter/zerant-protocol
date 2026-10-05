import assert from "node:assert/strict";
import test from "node:test";
import { accountSecuritySummary } from "./account-settings";

test("account security summary reports access redundancy without device profiling", () => {
  const summary = accountSecuritySummary(
    [{ id: "p1", created_at: "2026-10-05T00:00:00Z", updated_at: "2026-10-05T00:00:00Z", last_used_at: null }],
    [{ method: "wallet_message", chain: "zcash:testnet", created_at: "2026-10-05T00:00:00Z" }],
    [{ id: "s1", auth_method: "passkey", current: true, created_at: "2026-10-05T00:00:00Z", last_seen_at: "2026-10-05T00:00:00Z", expires_at: "2026-10-12T00:00:00Z" }],
  );
  assert.deepEqual(summary, { accessMethods: 2, passkeys: 1, zcashMethods: 1, activeSessions: 1, hasRedundantAccess: true });
  assert.equal("device" in summary, false);
  assert.equal("location" in summary, false);
});

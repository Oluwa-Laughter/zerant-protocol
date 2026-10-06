import assert from "node:assert/strict";
import test from "node:test";
import {
  accountExportCompletionMessage,
  accountSecuritySummary,
  accountSecuritySummaryFromCounts,
} from "./account-settings";

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


test("count-based security summary stays consistent after client updates", () => {
  assert.deepEqual(
    accountSecuritySummaryFromCounts(2, 0, 1),
    { accessMethods: 2, passkeys: 2, zcashMethods: 0, activeSessions: 1, hasRedundantAccess: true },
  );
  assert.deepEqual(
    accountSecuritySummaryFromCounts(1, 0, 1),
    { accessMethods: 1, passkeys: 1, zcashMethods: 0, activeSessions: 1, hasRedundantAccess: false },
  );
});


test("account export completion message reports every truncation combination", () => {
  assert.equal(accountExportCompletionMessage({ activity_complete: true, payments_complete: true }), "Your Zerant data export is ready.");
  assert.match(accountExportCompletionMessage({ activity_complete: false, payments_complete: true }), /Older account activity/);
  assert.match(accountExportCompletionMessage({ activity_complete: true, payments_complete: false }), /Older Zcash payments/);
  const both = accountExportCompletionMessage({ activity_complete: false, payments_complete: false });
  assert.match(both, /Older account activity/);
  assert.match(both, /older Zcash payments/);
});

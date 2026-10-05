import assert from "node:assert/strict";
import test from "node:test";
import { operationalProbeStatus, type OperationalHealthSnapshot } from "./operational-health";

function snapshot(overrides: Partial<OperationalHealthSnapshot> = {}): OperationalHealthSnapshot {
  return {
    healthy: true,
    attention_required: false,
    pending_verifications: 0,
    overdue_verifications: 0,
    webhook_pending: 0,
    webhook_dead: 0,
    maintenance_last_success_at: "2026-10-05T00:00:00Z",
    maintenance_stale: false,
    zcash: {
      configured: true,
      network: "testnet",
      available: true,
      synced: true,
      checked_at: "2026-10-05T00:00:00Z",
      last_success_at: "2026-10-05T00:00:00Z",
      stale: false,
    },
    ...overrides,
  };
}

test("operational probe succeeds only for a clean snapshot", () => {
  assert.equal(operationalProbeStatus(snapshot()), 200);
  assert.equal(operationalProbeStatus(snapshot({ healthy: false })), 503);
  assert.equal(operationalProbeStatus(snapshot({ attention_required: true })), 503);
});

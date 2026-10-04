import assert from "node:assert/strict";
import test from "node:test";
import {
  isWebhookQueueMessage,
  webhookQueueRetrySeconds,
} from "./webhook-queue";

test("webhook retry schedule is bounded and non-decreasing", () => {
  const values = Array.from({ length: 12 }, (_, index) =>
    webhookQueueRetrySeconds(index + 1),
  );
  assert.deepEqual(values.slice(0, 7), [60, 300, 900, 3600, 21600, 43200, 86400]);
  assert.equal(values[11], 86400);
});

test("webhook queue message requires a UUID request id", () => {
  assert.equal(
    isWebhookQueueMessage({ requestId: "11111111-1111-4111-8111-111111111111" }),
    true,
  );
  assert.equal(isWebhookQueueMessage({ requestId: "not-a-request" }), false);
  assert.equal(isWebhookQueueMessage(null), false);
});

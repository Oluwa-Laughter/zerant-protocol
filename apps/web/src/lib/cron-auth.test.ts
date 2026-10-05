import assert from "node:assert/strict";
import test from "node:test";
import { isAuthorizedCronRequest } from "./cron-auth";

const secret = "s".repeat(48);

test("cron authorization requires the exact configured bearer secret", () => {
  assert.equal(isAuthorizedCronRequest("Bearer " + secret, secret), true);
  assert.equal(isAuthorizedCronRequest("Bearer " + "x".repeat(48), secret), false);
  assert.equal(isAuthorizedCronRequest("Basic " + secret, secret), false);
  assert.equal(isAuthorizedCronRequest(null, secret), false);
});

test("cron authorization fails closed for missing or weak configuration", () => {
  assert.equal(isAuthorizedCronRequest("Bearer " + secret, undefined), false);
  assert.equal(isAuthorizedCronRequest("Bearer short", "short"), false);
});

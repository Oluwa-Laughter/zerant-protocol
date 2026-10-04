import { test } from "node:test";
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import { DemoDashboard } from "./demo-dashboard";

test("compound console presents fixture authority and complete disclosure", () => {
  const html = renderToStaticMarkup(<DemoDashboard />);
  for (const text of ["Compose one explicit request.", "Complete outbound requirement summary", "unauthenticated fixture", "Exact scores", "wallet history", "Recorded live RPC capabilities", "no live browser connection"]) {
    assert.ok(html.includes(text), `Missing boundary: ${text}`);
  }
  assert.ok(html.includes("No response; awaiting full-request approval"));
  assert.ok(!html.includes("payment completed"));
});

import { test } from "node:test";
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import { ConsentPanel } from "./consent-panel";
import { previewRequest } from "../lib/demo-data";
test("consent review renders requester, precise predicate, limits and correlation metadata", () => {
 const html = renderToStaticMarkup(<ConsentPanel request={previewRequest} />);
 for (const text of ["https://grants.example", "reputation.threshold", "score ≥ 40", "Not shared", "Exact reputation score", "Source credentials and contribution history", "Audience subject public key", "Credential / revocation IDs", "challenge, nonce", "not authenticated", "not a signed payload", "not ZK"]) assert.ok(html.includes(text), `Missing consent boundary: ${text}`);
 assert.ok(!html.includes("<input"), "Consent must not hide preselected attributes");
});

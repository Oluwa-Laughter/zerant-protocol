import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { PasskeyAccess } from "./passkey-access";

test("passkey access offers account-free sign-in with Zerant ID fallback", () => {
  const html = renderToStaticMarkup(<PasskeyAccess />);
  assert.ok(html.includes("Create a Zerant account"));
  assert.ok(html.includes("Continue with passkey"));
  assert.ok(html.includes("Use Zerant ID instead"));
  assert.ok(html.includes("Continue with Zerant ID"));
});

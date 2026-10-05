import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { PasskeyAccess } from "./passkey-access";

test("passkey access offers account-free sign-in with Zerant ID fallback", () => {
  const html = renderToStaticMarkup(<PasskeyAccess />);
  assert.ok(html.includes("Create Zerant account"));
  assert.ok(html.includes("Sign in with passkey"));
  assert.ok(html.includes("Use your Zerant ID to sign in"));
  assert.ok(html.includes("Continue with Zerant ID"));
});

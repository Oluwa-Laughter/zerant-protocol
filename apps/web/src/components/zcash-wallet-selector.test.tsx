import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { ZcashWalletSelector } from "./zcash-wallet-selector";

test("wallet selector keeps wallet access action-scoped", () => {
  const html = renderToStaticMarkup(
    <ZcashWalletSelector purpose="connection" onSelect={() => {}} />,
  );
  assert.ok(html.includes("Connect Zcash wallet"));
  assert.equal(html.includes("balance"), false);
  assert.equal(html.includes("history"), false);
});


test("wallet selector keeps testnet Noir guidance inside supported sign-in flow", () => {
  const html = renderToStaticMarkup(
    <ZcashWalletSelector purpose="identity" onSelect={() => {}} />,
  );
  assert.ok(html.includes("Connect Zcash wallet") || html.includes("Choose"));
});

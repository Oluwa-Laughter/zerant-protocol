import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { ZcashConnect } from "./zcash-connect";

test("wallet connection presents the explicit choice before secondary recovery", () => {
  const html = renderToStaticMarkup(<ZcashConnect purpose="connection" />);
  const connectButton = html.indexOf("Connect a direct wallet");
  const recovery = html.indexOf("Having trouble connecting?");
  const details = html.indexOf("Connection details");
  assert.ok(connectButton >= 0);
  assert.ok(recovery > connectButton);
  assert.ok(details > recovery);
  assert.ok(html.includes("<details"));
  assert.equal(html.includes("<details open"), false);
  assert.equal(html.includes("Authorized · testnet ready"), false);
});

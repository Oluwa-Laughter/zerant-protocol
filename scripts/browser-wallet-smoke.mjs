// Browser-level Noir connection check using an isolated, simulated provider.
// This never grants wallet authority or submits a payment. A real extension
// approval still needs a human test on the configured Zcash network.
// NOIR_EXTENSION_DIR may point to an unpacked official Testnet Noir build; in
// that mode a fresh profile checks detection and the empty-wallet response only.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const baseUrl = process.argv[2] ?? "http://localhost:3000";
const browserBinary = process.env.CHROMIUM_BIN ?? "google-chrome";
const extensionDir = process.env.NOIR_EXTENSION_DIR;
const profile = await mkdtemp(join(tmpdir(), "zerant-wallet-smoke-"));
const browserArgs = [
  "--headless=new", "--no-first-run", "--no-default-browser-check",
  "--remote-debugging-port=0", `--user-data-dir=${profile}`, "about:blank",
];
if (extensionDir) browserArgs.unshift(`--disable-extensions-except=${extensionDir}`, `--load-extension=${extensionDir}`);
const browser = spawn(browserBinary, browserArgs, { stdio: "ignore" });

function delay(ms) { return new Promise((resolve) => setTimeout(resolve, ms)); }

async function debuggerPort() {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    try {
      const [port] = (await readFile(join(profile, "DevToolsActivePort"), "utf8")).split("\n");
      if (Number.isInteger(Number(port)) && Number(port) > 0) return Number(port);
    } catch { /* Chrome has not started yet. */ }
    await delay(100);
  }
  throw new Error("Chromium did not start its debugging endpoint.");
}

async function pageSocket(port) {
  for (let attempt = 0; attempt < 30; attempt += 1) {
    const pages = await fetch(`http://127.0.0.1:${port}/json/list`).then((response) => response.json());
    const page = pages.find((entry) => entry.type === "page");
    if (page?.webSocketDebuggerUrl) return page.webSocketDebuggerUrl;
    await delay(100);
  }
  throw new Error("Chromium did not create a page target.");
}

function connectCdp(url) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(url);
    const pending = new Map();
    let nextId = 1;
    ws.addEventListener("error", reject, { once: true });
    ws.addEventListener("open", () => resolve({
      close: () => ws.close(),
      send(method, params = {}) {
        const id = nextId++;
        return new Promise((done, fail) => {
          pending.set(id, { done, fail });
          ws.send(JSON.stringify({ id, method, params }));
        });
      },
    }), { once: true });
    ws.addEventListener("message", (event) => {
      const reply = JSON.parse(event.data);
      if (!reply.id || !pending.has(reply.id)) return;
      const { done, fail } = pending.get(reply.id);
      pending.delete(reply.id);
      if (reply.error) fail(new Error(`${reply.error.message} (${reply.error.code})`));
      else done(reply.result);
    });
  });
}

const simulatedProvider = `(() => {
  const handlers = new Map();
  window.__zerantWalletCalls = [];
  window.__pageErrors = [];
  window.addEventListener("error", (event) => window.__pageErrors.push(event.message));
  window.addEventListener("unhandledrejection", (event) => window.__pageErrors.push(String(event.reason)));
  window.__emitWalletEvent = (event) => handlers.get(event)?.forEach((handler) => handler());
  window.noirwallet = {
    isNoirWallet: true,
    version: "1.0.27",
    zcash: {
      on(event, handler) { handlers.set(event, [...(handlers.get(event) ?? []), handler]); },
      removeListener(event, handler) { handlers.set(event, (handlers.get(event) ?? []).filter((candidate) => candidate !== handler)); },
      async request({ method }) {
        window.__zerantWalletCalls.push(method);
        if (method === "zcash_requestAccounts") {
          if (location.search.includes("reject")) throw { code: 4001, message: "User rejected" };
          if (location.search.includes("mainnet")) return { shielded: "u1wrongnetwork", transparent: "t1wrongnetwork", accounts: [{}] };
          return { shielded: "utest1connected", transparent: "tmconnected", accounts: [{}] };
        }
        if (method === "zcash_getAccounts") return null;
        if (method === "zcash_disconnect") return null;
        throw new Error("Unexpected wallet method: " + method);
      },
    },
  };
})();`;

let cdp;
try {
  const port = await debuggerPort();
  cdp = await connectCdp(await pageSocket(port));
  await cdp.send("Page.enable");
  await cdp.send("Runtime.enable");
  if (!extensionDir) await cdp.send("Page.addScriptToEvaluateOnNewDocument", { source: simulatedProvider });

  async function evaluate(expression) {
    const result = await cdp.send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    if (result.exceptionDetails) throw new Error(result.exceptionDetails.text);
    return result.result.value;
  }
  async function waitFor(expression) {
    for (let attempt = 0; attempt < 300; attempt += 1) {
      if (await evaluate(expression)) return;
      await delay(100);
    }
    throw new Error(`Browser condition timed out: ${expression}`);
  }
  async function clickButton(label) {
    const point = await evaluate(`(() => {
      const button = [...document.querySelectorAll("button")].find((element) => element.textContent?.includes(${JSON.stringify(label)}));
      if (!button) return null;
      button.scrollIntoView({ block: "center", behavior: "instant" });
      const rect = button.getBoundingClientRect();
      return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 };
    })()`);
    assert.ok(point, `Button not found: ${label}`);
    await cdp.send("Input.dispatchMouseEvent", { type: "mousePressed", x: point.x, y: point.y, button: "left", clickCount: 1 });
    await cdp.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: point.x, y: point.y, button: "left", clickCount: 1 });
  }

  if (extensionDir) {
    await cdp.send("Page.navigate", { url: `${baseUrl}/vault` });
    await waitFor("document.readyState === 'complete' && !!window.noirwallet");
    await waitFor(`(() => {
      const button = [...document.querySelectorAll("button")].find((element) => element.textContent?.includes("Choose sign-in wallet"));
      return !!button && Object.keys(button).some((key) => key.startsWith("__reactProps$"));
    })()`);
    await clickButton("Choose sign-in wallet");
    await waitFor(`!![...document.querySelectorAll("button")].find((button) => button.textContent?.includes("Noir Wallet"))`);
    console.log("Official extension: provider detected; chooser displays Noir Wallet.");
    await clickButton("Noir Wallet");
    await waitFor("document.body.innerText.includes('No wallets available to authorize') || document.body.innerText.includes('no account ready to share')");
    const targets = await fetch(`http://127.0.0.1:${port}/json/list`).then((response) => response.json());
    console.log("Browser targets after Connect:", targets.map((target) => `${target.type}: ${target.url.startsWith("chrome-extension://") ? "wallet extension" : target.url.startsWith(baseUrl) ? "Zerant" : "other"}`).join(", "));
    console.log("Zerant result:", (await evaluate("document.body.innerText")).slice(-400));
  } else for (const [scenario, outcome] of [
    ["success", "Wallet connected"],
    ["reject", "closed or rejected"],
    ["mainnet", "does not match Zerant"],
  ]) {
    await cdp.send("Page.navigate", { url: `${baseUrl}/vault?walletSmoke=${scenario}` });
    try {
      await waitFor(`(() => {
        const button = [...document.querySelectorAll("button")].find((element) => element.textContent?.includes("Choose sign-in wallet"));
        return !!button && Object.keys(button).some((key) => key.startsWith("__reactProps$"));
      })()`);
    } catch (error) {
      console.error("Page diagnostics:", await evaluate("JSON.stringify({ errors: window.__pageErrors, scripts: [...document.scripts].slice(0, 8).map((script) => script.src), state: document.readyState })"));
      throw error;
    }
    assert.deepEqual(await evaluate("window.__zerantWalletCalls"), [], "Mount must not query the wallet");
    await clickButton("Choose sign-in wallet");
    try {
      await waitFor(`!![...document.querySelectorAll("button")].find((button) => button.textContent?.includes("Noir Wallet"))`);
    } catch (error) {
      console.error("Chooser state:", await evaluate("document.body.innerText.slice(0, 1400)"));
      throw error;
    }
    assert.deepEqual(await evaluate("window.__zerantWalletCalls"), [], "Chooser must only detect the wallet");
    await clickButton("Noir Wallet");
    await waitFor(`document.body.innerText.includes(${JSON.stringify(outcome)})`);
    const calls = await evaluate("window.__zerantWalletCalls");
    assert.equal(calls[0], "zcash_requestAccounts", "Click must start interactive authorization");
    assert.equal(calls.filter((method) => method === "zcash_requestAccounts").length, 1, "Connect must run once");
    if (scenario === "reject") assert.deepEqual(calls, ["zcash_requestAccounts"], "Rejection must not trigger recovery RPCs");
    if (scenario === "mainnet") assert.deepEqual(calls, ["zcash_requestAccounts", "zcash_disconnect"], "Wrong network must be disconnected");
    console.log(`${scenario}: ${calls.join(", ")} → ${outcome}`);
  }
} finally {
  cdp?.close();
  browser.kill();
  await delay(200);
  await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}

// Record the live public Zerant pages in Chromium. This script never creates
// accounts, injects wallet providers, or invents credentials or payments.
import { spawn, spawnSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

const origin = process.argv[2] ?? "https://zerant.vercel.app";
const destination = process.argv[3] ?? "docs/assets/demo-public-walkthrough.mp4";
const browserBinary = process.env.CHROMIUM_BIN ?? "google-chrome";
const working = await mkdtemp(join(tmpdir(), "zerant-public-demo-"));
const profile = join(working, "profile");
const frames = join(working, "frames");
await mkdir(frames);
await mkdir(dirname(destination), { recursive: true });
const browser = spawn(browserBinary, [
  "--headless=new", "--no-first-run", "--no-default-browser-check",
  "--window-size=1280,720", "--remote-debugging-port=0",
  `--user-data-dir=${profile}`, "about:blank",
], { stdio: "ignore" });

const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function port() {
  for (let i = 0; i < 100; i += 1) {
    try {
      const [value] = (await readFile(join(profile, "DevToolsActivePort"), "utf8")).split("\n");
      if (Number(value) > 0) return Number(value);
    } catch { /* Browser is starting. */ }
    await delay(100);
  }
  throw new Error("Chromium debugging endpoint unavailable.");
}

let ws;
let nextId = 1;
const pending = new Map();
async function send(method, params = {}) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });
}
async function evaluate(expression) {
  const reply = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
  if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.text);
  return reply.result.value;
}
async function waitFor(expression) {
  for (let i = 0; i < 300; i += 1) {
    if (await evaluate(expression)) return;
    await delay(100);
  }
  throw new Error(`Page did not become ready: ${expression}`);
}

let frame = 0;
async function capture() {
  const reply = await send("Page.captureScreenshot", { format: "jpeg", quality: 80, captureBeyondViewport: false });
  await writeFile(join(frames, `${String(frame++).padStart(5, "0")}.jpg`), Buffer.from(reply.data, "base64"));
}
async function hold(seconds, scroll = false) {
  const count = Math.round(seconds * 8);
  for (let i = 0; i < count; i += 1) {
    if (scroll && i % 3 === 0) await evaluate("window.scrollBy(0, 34)");
    await capture();
    await delay(125);
  }
}

try {
  const debuggingPort = await port();
  const pages = await fetch(`http://127.0.0.1:${debuggingPort}/json/list`).then((response) => response.json());
  const target = pages.find((page) => page.type === "page");
  if (!target) throw new Error("Chromium page target unavailable.");
  ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    ws.addEventListener("open", resolve, { once: true });
    ws.addEventListener("error", reject, { once: true });
  });
  ws.addEventListener("message", (event) => {
    const reply = JSON.parse(event.data);
    if (!reply.id || !pending.has(reply.id)) return;
    const { resolve, reject } = pending.get(reply.id);
    pending.delete(reply.id);
    if (reply.error) reject(new Error(reply.error.message));
    else resolve(reply.result);
  });
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 720, deviceScaleFactor: 1, mobile: false });

  for (const [route, seconds, scroll] of [
    ["/", 13, true],
    ["/vault", 11, true],
    ["/zcash", 10, true],
    ["/issuers", 8, true],
    ["/verifier", 8, true],
  ]) {
    await send("Page.navigate", { url: origin + route });
    await waitFor("document.readyState === 'complete' && !!document.querySelector('main')");
    await hold(seconds, scroll);
    console.log(`Recorded ${route}`);
  }

  const encoding = spawnSync("ffmpeg", [
    "-y", "-loglevel", "error", "-framerate", "8", "-i", join(frames, "%05d.jpg"),
    "-c:v", "mpeg4", "-pix_fmt", "yuv420p", "-q:v", "4", "-movflags", "+faststart", destination,
  ], { encoding: "utf8" });
  if (encoding.status !== 0) throw new Error(encoding.stderr || "Video encoding failed.");
  console.log(`Saved ${destination} from ${frame} live browser frames.`);
} finally {
  ws?.close();
  browser.kill();
  await delay(200);
  await rm(working, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}

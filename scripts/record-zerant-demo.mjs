// Capture actual Chromium compositor frames and interactions, not a slide montage.
// Connect to an already authenticated browser. No auth bypass or wallet simulation.
import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { connect, delay } from './demo-cdp.mjs';
const [configPath, outputPath] = process.argv.slice(2);
if (!configPath || !outputPath) throw new Error('Usage: node scripts/record-zerant-demo.mjs CONFIG OUTPUT_DIRECTORY');
const config = JSON.parse(await readFile(configPath, 'utf8'));
const output = resolve(outputPath);
await mkdir(output, { recursive: true });
if ((await readdir(output)).length) throw new Error('Capture directory must be empty; preserve previous takes in another directory.');
if (!config.scenes?.length || config.scenes.some(s=>!(s.duration>0))) throw new Error('Invalid scene timeline');
const browser = await connect(process.env.DEMO_CDP ?? 'http://127.0.0.1:9225', process.env.DEMO_TARGET);
const { call, evaluate, waitFor } = browser;
let cursor = [30, 30];
const pointer = `<svg xmlns="http://www.w3.org/2000/svg" width="12" height="18" viewBox="0 0 12 18"><path d="M1 1v13l3.5-3L7 17l2-1-2.5-6H11Z" fill="white" stroke="black" stroke-width="1"/></svg>`;
async function installPointer() {
  await evaluate(`(()=>{document.getElementById('zerant-demo-pointer')?.remove();const p=document.createElement('div');p.id='zerant-demo-pointer';p.innerHTML=${JSON.stringify(pointer)};Object.assign(p.style,{position:'fixed',left:'30px',top:'30px',width:'12px',height:'18px',zIndex:'2147483647',pointerEvents:'none'});document.body.append(p)})()`);
  cursor = [30, 30];
}
async function move(x, y) {
  const start = [...cursor];
  for (let i = 1; i <= 18; i++) {
    const f = i / 18; const t = f * f * (3 - 2 * f);
    const px = start[0] + (x - start[0]) * t; const py = start[1] + (y - start[1]) * t;
    await evaluate(`(()=>{const p=document.getElementById('zerant-demo-pointer');if(p){p.style.left='${px}px';p.style.top='${py}px'}})()`);
    await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x: px, y: py });
    await delay(25);
  }
  cursor = [x, y];
}
async function action(a) {
  if (a.kind === 'scroll') {
    await move(a.x ?? 1700, a.y ?? 800);
    await call('Input.dispatchMouseEvent', { type: 'mouseWheel', x: cursor[0], y: cursor[1], deltaX: 0, deltaY: a.delta });
  } else if (a.kind === 'click') {
    const target = await evaluate(`(()=>{const e=${a.selector ? `document.querySelector(${JSON.stringify(a.selector)})` : `[...document.querySelectorAll('button,a')].find(e=>e.innerText.trim()===${JSON.stringify(a.text)})`};if(!e||e.disabled)return null;e.scrollIntoView({block:'nearest',behavior:'instant'});const r=e.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()`);
    if (!target) throw new Error(`Click target unavailable: ${a.text ?? a.selector}`);
    await move(target.x, target.y); await delay(200);
    await call('Input.dispatchMouseEvent', { type: 'mousePressed', x: target.x, y: target.y, button: 'left', clickCount: 1 });
    await call('Input.dispatchMouseEvent', { type: 'mouseReleased', x: target.x, y: target.y, button: 'left', clickCount: 1 });
  } else if (a.kind === 'focus') {
    await evaluate(`document.querySelector(${JSON.stringify(a.selector)})?.scrollIntoView({block:'center',behavior:'instant'})`);
  } else if (a.kind === 'wait') await waitFor(a.expression);
  else if (a.kind === 'fill') {
    await action({ kind: 'click', selector: a.selector });
    const value = a.value ?? (await readFile(process.env[a.valueFileEnv], 'utf8')).trim();
    await evaluate(`document.querySelector(${JSON.stringify(a.selector)}).focus()`);
    await call('Input.insertText', { text: value });
    await delay(150);
    if (!await evaluate(`document.querySelector(${JSON.stringify(a.selector)}).value===${JSON.stringify(value)}`)) throw new Error('Browser form did not accept the entered text');
  } else throw new Error(`Unknown action ${a.kind}`);
}
let total = 0; const manifest = []; const scenes = [];
async function checkpoint() {
  // Explicit final frame gives concat demuxer the last hold duration.
  const lines = ['ffconcat version 1.0'];
  for (let i = 0; i < manifest.length; i++) {
    lines.push(`file '${manifest[i].file}'`, `duration ${((manifest[i+1]?.time ?? total) - manifest[i].time).toFixed(6)}`);
  }
  lines.push(`file '${manifest.at(-1).file}'`);
  await writeFile(join(output, 'frames.ffconcat'), lines.join('\n')+'\n');
  await writeFile(join(output, 'capture.json'), JSON.stringify({ duration: total, cursor: '12x18 pixels; no click effects', scenes, frames: manifest.length }, null, 2));
}
try {
  await call('Page.enable'); await call('Runtime.enable'); await call('Page.bringToFront');
  for (const scene of config.scenes) {
    await call('Emulation.setDeviceMetricsOverride', { width: scene.width ?? 1920, height: scene.height ?? 1080, deviceScaleFactor: 1, mobile: false });
    if (!scene.reuseCurrent) await call('Page.navigate', { url: config.origin + scene.route });
    else if (!await evaluate(`location.href===${JSON.stringify(config.origin + scene.route)}`)) throw new Error('Current page does not match the scene route');
    await waitFor(`document.readyState==='complete' && document.body.innerText.includes(${JSON.stringify(scene.expect)})`);
    if (scene.authenticated) await waitFor("!document.body.innerText.includes('Continue with passkey') && !document.body.innerText.includes('Connect to Zerant') && !document.body.innerText.includes('Sign in required')");
    await delay(700); await installPointer();
    for (const a of scene.prepare ?? []) await action(a);
    await delay(300);
    const first = await call('Page.captureScreenshot', { format: 'jpeg', quality: 92, captureBeyondViewport: false });
    const started = performance.now(); let last = 0; let writing = Promise.resolve();
    const append = (data, time) => {
      const name = `${String(manifest.length).padStart(6,'0')}.jpg`;
      manifest.push({ file: name, time: total + time });
      writing = writing.then(() => writeFile(join(output, name), Buffer.from(data, 'base64')));
    };
    append(first.data, 0);
    browser.on('Page.screencastFrame', (frame) => {
      const time = (performance.now() - started) / 1000;
      if (time < scene.duration && time - last >= 1 / 30) { append(frame.data, time); last = time; }
      void call('Page.screencastFrameAck', { sessionId: frame.sessionId });
    });
    let polling = scene.captureScreenshots === true;
    let poller;
    if (polling) {
      // Some headless compositor surfaces clip tall mobile screencasts.
      // Full-viewport screenshot sampling preserves actual page painting.
      poller = (async()=>{
        while (polling) {
          const shot = await call('Page.captureScreenshot', {format:'jpeg',quality:92,captureBeyondViewport:false});
          const time = (performance.now()-started)/1000;
          if (time < scene.duration) append(shot.data,time);
          await delay(50);
        }
      })();
    } else await call('Page.startScreencast', { format: 'jpeg', quality: 92, maxWidth: 1920, maxHeight: 1080, everyNthFrame: 1 });
    for (const a of scene.actions ?? []) {
      await delay(Math.max(0, a.at * 1000 - (performance.now() - started)));
      await action(a);
      if ((performance.now() - started) / 1000 > scene.duration) throw new Error('Actions exceeded scene duration');
    }
    await delay(Math.max(0, scene.duration * 1000 - (performance.now() - started)));
    polling = false; if (poller) await poller;
    else await call('Page.stopScreencast'); browser.on('Page.screencastFrame', () => {}); await writing;
    scenes.push({ name: scene.name, start: total, duration: scene.duration, frames: manifest.length });
    total += scene.duration;
    await checkpoint();
    console.log(`Captured ${scene.name}: ${scene.duration}s`);
  }
  await checkpoint();
} finally {
  await evaluate("document.getElementById('zerant-demo-pointer')?.remove()").catch(()=>{});
  await call('Emulation.clearDeviceMetricsOverride').catch(()=>{});
  browser.close();
}

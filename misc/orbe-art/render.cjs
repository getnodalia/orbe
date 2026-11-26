// SPDX-License-Identifier: MIT
// Renders Orbe's PNGs from the SVGs that generate.py wrote: node misc/orbe-art/render.cjs
// Needs playwright with a Chromium. Afterwards, the four logo PNGs are reduced to 256 colours with
// `convert X.png -colors 256 PNG8:X.png` (flat art loses nothing; upstream's PCK packer test caps the
// size of these files). The Windows .ico files are made from icon.png with ImageMagick.
const { chromium } = require("playwright");
const { readFileSync } = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "../..");
const svg = (p) => readFileSync(path.join(root, p), "utf8");
const jobs = [
  ["misc/logo/icon.svg", "misc/logo/icon.png", 256, 256],
  ["misc/logo/icon_outlined.svg", "misc/logo/icon_outlined.png", 256, 256],
  ["misc/logo/logo.svg", "misc/logo/logo.png", 1024, null],
  ["misc/logo/logo_outlined.svg", "misc/logo/logo_outlined.png", 512, null],
];
(async () => {
const browser = await chromium.launch();
const page = await browser.newPage();
async function shot(markup, out, w, h, bg = "transparent") {
  await page.setViewportSize({ width: w, height: h });
  await page.setContent(`<body style="margin:0;background:${bg}">${markup}</body>`);
  await page.screenshot({ path: path.join(root, out), omitBackground: bg === "transparent" });
  console.log("wrote", out);
}
for (const [src, out, w, h] of jobs) {
  const m = /width="(\d+)" height="(\d+)"/.exec(svg(src));
  const ww = w, hh = h ?? Math.round((w * +m[2]) / +m[1]);
  const markup = svg(src).replace(/width="\d+" height="\d+"/, `width="${ww}" height="${hh}"`);
  await shot(markup, out, ww, hh);
}
// The splash: the lockup, centred on the NDL dark window colour.
const lock = svg("misc/logo/logo.svg").replace(/width="\d+" height="\d+"/, 'width="520" height="193"');
await shot(`<div style="width:800px;height:600px;display:grid;place-items:center">${lock}</div>`, "main/splash.png", 800, 600, "#0B0C10");
await browser.close();
})();

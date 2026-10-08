// Phone-sized landscape run of the live game with touch emulation; screenshots to argv[2].
const puppeteer = require(process.env.PPTR);
const out = process.argv[2];
const url = process.argv[3] || "https://rabbiveesh.github.io/nat-han-adventures/?touch=1";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
(async () => {
  const browser = await puppeteer.launch({
    executablePath: "/usr/bin/google-chrome",
    headless: "new",
    args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--autoplay-policy=no-user-gesture-required", "--mute-audio"],
  });
  const page = await browser.newPage();
  page.on("console", (m) => { if (/error|panic/i.test(m.text())) console.log("console:", m.text().slice(0, 300)); });
  await page.emulate({
    viewport: { width: 844, height: 390, isMobile: true, hasTouch: true, isLandscape: true, deviceScaleFactor: 2 },
    userAgent: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
  });
  await page.goto(url, { waitUntil: "load", timeout: 120000 });
  await page.waitForFunction(() => /tap|click/.test(document.getElementById("start-msg")?.textContent || "") || !document.getElementById("start"), { timeout: 180000 });
  const shot = async (n) => page.screenshot({ path: `${out}/${n}.png` });
  await shot("0-start");
  const t = page.touchscreen;
  await t.tap(422, 195); await sleep(2500); await shot("1-title");
  await t.tap(560, 30); await sleep(1500); await shot("2-select");   // OK
  await t.tap(560, 30); await sleep(3500); await shot("3-level");    // OK -> level 1
  // Floating stick: thumb down low-left, slide right, hold.
  await t.touchStart(110, 330); await sleep(100); await t.touchMove(150, 332); await sleep(1200);
  await shot("4-running");
  // Jump while running (second finger on the right half), then toot.
  await t.tap(740, 300); await sleep(150); await t.tap(740, 300); await sleep(300);
  await shot("5-jump");
  await t.touchEnd(); await sleep(1500); await shot("6-after");
  const cls = await page.evaluate(() => document.body.className);
  console.log("body class:", cls);
  await browser.close();
})().catch((e) => { console.error(e); process.exit(1); });

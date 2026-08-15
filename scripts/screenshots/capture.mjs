/**
 * Regenerates every screenshot in `screenshots/`.
 *
 * The UI is the real one: Vite serves the same `src/` the desktop build ships,
 * and the only thing replaced is the IPC bridge — `tauri-mock.js` answers
 * `invoke()` with a fixture, so the window is full of plausible profiles,
 * repositories and problems instead of an empty first-run state. Chromium is
 * sized to the app's own default window (see `tauri.conf.json`) at 2x, and
 * encodes the WebP itself, so no image dependency is needed.
 *
 *   pnpm screenshots
 */
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const OUT = join(ROOT, "screenshots");
const MOCK = join(ROOT, "scripts", "screenshots", "tauri-mock.js");

/** The desktop window's own size, so a shot frames what a user actually sees. */
const VIEWPORT = { width: 920, height: 660 };
const SCALE = 2;
const WEBP_QUALITY = 0.9;

/** Scrolls `locator` to the top of whichever pane it lives in. */
async function scrollTo(page, locator, offset = 20) {
  await locator.first().evaluate((el, gap) => {
    const pane = el.closest(".overflow-y-auto");
    if (!pane) return;
    pane.scrollTop +=
      el.getBoundingClientRect().top - pane.getBoundingClientRect().top - gap;
  }, offset);
  // One frame for the scroll to land before the shot.
  await page.waitForTimeout(120);
}

/** Chromium encodes the WebP; `page` is any loaded page, used as the encoder. */
async function toWebp(page, png) {
  const base64 = await page.evaluate(
    async ({ data, quality }) => {
      const img = new Image();
      img.src = `data:image/png;base64,${data}`;
      await img.decode();
      const canvas = document.createElement("canvas");
      canvas.width = img.naturalWidth;
      canvas.height = img.naturalHeight;
      canvas.getContext("2d").drawImage(img, 0, 0);
      const url = canvas.toDataURL("image/webp", quality);
      if (!url.startsWith("data:image/webp")) {
        throw new Error("this Chromium cannot encode WebP");
      }
      return url.slice(url.indexOf(",") + 1);
    },
    { data: png.toString("base64"), quality: WEBP_QUALITY },
  );
  return Buffer.from(base64, "base64");
}

async function captureTheme(browser, url, theme) {
  const context = await browser.newContext({
    viewport: VIEWPORT,
    deviceScaleFactor: SCALE,
    colorScheme: theme,
    locale: "en-US",
  });
  await context.addInitScript({ path: MOCK });
  await context.addInitScript(
    ([pref]) => {
      localStorage.setItem("theme-preference", pref);
      localStorage.setItem("language-preference", "en");
    },
    [theme],
  );

  const page = await context.newPage();
  const complaints = [];
  page.on("console", (msg) => {
    if (msg.type() === "error") complaints.push(msg.text());
  });
  page.on("pageerror", (e) => complaints.push(String(e)));

  const shot = async (n) => {
    const png = await page.screenshot({ type: "png" });
    const file = join(OUT, `${theme}${n}.webp`);
    await writeFile(file, await toWebp(page, png));
    console.log(`  ${theme}${n}.webp`);
  };

  await page.goto(url, { waitUntil: "networkidle" });
  // Transitions are per-element and short, but a shot taken mid-fade is a
  // different image every run.
  await page.addStyleTag({
    content: "*,*::before,*::after{transition:none!important;animation:none!important}",
  });

  // 1 — the profile list, with an active profile and one carrying problems.
  await page.getByRole("heading", { name: "Work", exact: true }).waitFor();
  await page.getByText("3 issues").waitFor();
  await shot(1);

  // 2 — settings: theme, autostart, language, and the identity guard rails.
  await page.getByTitle("OAuth Settings").click();
  await page.getByText("Identity guard").waitFor();
  await shot(2);
  await page.keyboard.press("Escape");

  // 3 — the profile editor, with all three platforms connected.
  await page.getByRole("button", { name: "Edit" }).first().click();
  await page.getByRole("heading", { name: "Edit Profile" }).waitFor();
  await page.getByText(/repositories ·/).first().waitFor();
  await shot(3);

  // 4 — which connected identity Git uses, above the folders it watches.
  await scrollTo(
    page,
    page.locator(".panel").filter({ hasText: "Default Git Identity" }),
  );
  await shot(4);

  // 5 — the scanned repositories and what the doctor found wrong with them.
  await scrollTo(page, page.getByRole("heading", { name: "Doctor" }));
  await shot(5);
  await page.keyboard.press("Escape");

  // 6 — connecting an account: GitHub's device flow and Bitbucket's token.
  await page.getByRole("button", { name: "New Profile" }).click();
  await page.getByRole("button", { name: "Connect with GitHub" }).click();
  await page.getByText("Enter this code on GitHub:").waitFor();
  await shot(6);

  await context.close();
  return complaints;
}

const server = await createServer({
  root: ROOT,
  configFile: join(ROOT, "vite.config.ts"),
  logLevel: "warn",
  server: { port: 1431, strictPort: true, open: false },
});
await server.listen();
const url = server.resolvedUrls.local[0];

const browser = await chromium.launch();
const complaints = [];
try {
  for (const theme of ["light", "dark"]) {
    await mkdir(OUT, { recursive: true });
    console.log(`${theme}:`);
    complaints.push(...(await captureTheme(browser, url, theme)));
  }
} finally {
  await browser.close();
  await server.close();
}

if (complaints.length > 0) {
  console.error("\nThe page complained while being captured:");
  for (const line of new Set(complaints)) console.error(`  ${line}`);
  process.exit(1);
}

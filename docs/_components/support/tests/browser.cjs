// Test the built book without starting a server or making external requests.
// NOTIST_PLAYWRIGHT may name a playwright-core module; NOTIST_CHROMIUM a browser.
const assert = require("node:assert/strict");
const fs = require("node:fs/promises");
const path = require("node:path");
const { chromium } = require(process.env.NOTIST_PLAYWRIGHT ?? "playwright");

(async () => {
  const root = path.resolve("dist/icexiv-data-book");
  const browser = await chromium.launch({ headless: true,
    ...(process.env.NOTIST_CHROMIUM ? { executablePath: process.env.NOTIST_CHROMIUM } : {}) });
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 1000 } });
    const errors = [];
    page.on("pageerror", error => errors.push(error.message));
    await page.route("**/*", async route => {
      const url = new URL(route.request().url());
      if (url.origin !== "http://notist.test") return route.abort();
      const file = path.resolve(root, "." + decodeURIComponent(url.pathname), url.pathname.endsWith("/") ? "index.html" : "");
      assert.ok(file.startsWith(root + path.sep));
      const type = file.endsWith(".js") ? "text/javascript" : file.endsWith(".css") ? "text/css" : "text/html";
      await route.fulfill({ body: await fs.readFile(file), contentType: type });
    });
    await page.goto("http://notist.test/support/");
    const host = page.locator('[notist-function$="/support/lib.notc.catalog"]');
    const cards = host.locator("article");
    await cards.first().waitFor();
    assert.equal(await cards.count(), 18);
    assert.equal(await host.locator("section").count(), 5);
    assert.equal(await host.locator("details[open]").count(), 0);
    await host.getByRole("button", { name: "部分 6", exact: true }).click();
    assert.equal(await host.locator("article:visible").count(), 6);
    assert.equal(await host.locator("section:visible").count(), 4);
    await host.getByRole("button", { name: "资料 1", exact: true }).click();
    assert.equal(await host.locator("article:visible").count(), 1);
    await host.getByRole("button", { name: "全部 18", exact: true }).click();
    const firstSummary = cards.first().locator("summary");
    await firstSummary.focus();
    await page.keyboard.press("Enter");
    assert.equal(await host.locator("details[open]").count(), 1);
    assert.equal(await host.getByRole("link", { name: "SqPack", exact: true }).getAttribute("href"), "http://notist.test/sqpack/");
    for (const href of await host.locator("a").evaluateAll(links => links.map(link => link.href))) {
      const destination = new URL(href).pathname;
      await fs.access(path.join(root, destination, "index.html"));
    }
    await firstSummary.click();
    await firstSummary.evaluate(element => element.blur());
    const colors = [];
    for (const theme of ["light", "dark"]) {
      await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
      colors.push(await cards.first().evaluate(card => getComputedStyle(card).backgroundColor));
    }
    assert.notEqual(colors[0], colors[1]);
    await page.evaluate(() => { document.documentElement.dataset.theme = "light"; });
    if (process.env.NOTIST_SCREENSHOTS) {
      await fs.mkdir(process.env.NOTIST_SCREENSHOTS, { recursive: true });
      await page.screenshot({ path: path.join(process.env.NOTIST_SCREENSHOTS, "support-desktop.png"), fullPage: true });
    }
    await page.setViewportSize({ width: 360, height: 800 });
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    assert.ok(await host.evaluate(element => {
      const grids = [...element.shadowRoot.querySelectorAll(".grid")];
      return grids.every(grid => getComputedStyle(grid).gridTemplateColumns.split(" ").length === 1);
    }));
    if (process.env.NOTIST_SCREENSHOTS) await page.screenshot({ path: path.join(process.env.NOTIST_SCREENSHOTS, "support-mobile.png"), fullPage: true });
    // Reconnect, source updates, safe text rendering, invalid input and recovery.
    await host.evaluate(element => {
      window.catalog = element;
      window.originalSource = element.getAttribute("notist-source");
      const parent = element.parentNode;
      element.remove(); parent.append(element);
    });
    assert.equal(await cards.count(), 18);
    await host.evaluate(element => element.setAttribute("notist-source", JSON.stringify([
      { title: "Test", items: [{ name: "<img src=x>", status: "ready", route: "tex/", summary: "Text", limit: "Limit" }] }
    ])));
    assert.equal(await host.locator("img").count(), 0);
    assert.equal(await host.locator("h3").textContent(), "<img src=x>");
    await host.evaluate(element => element.setAttribute("notist-source", "["));
    assert.equal(await host.getByRole("alert").count(), 1);
    await host.evaluate(element => element.setAttribute("notist-source", window.originalSource));
    assert.equal(await cards.count(), 18);
    assert.deepEqual(errors, []);
    console.log("Support catalog: render, filters, keyboard, links, themes, mobile, reconnect and input recovery passed.");
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });

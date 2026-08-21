import { expect, test, type Page } from "@playwright/test";
import path from "node:path";

const accessoryFixture = path.resolve(
  __dirname,
  "../../tests/fixtures/gene_presence_absence.Rtab.gz",
);

function collectBrowserErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(`pageerror: ${error.message}`));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(`console: ${message.text()}`);
  });
  return errors;
}

test("tracked gzip accessory input reaches a final embedding", async ({ page }) => {
  const errors = collectBrowserErrors(page);
  await page.goto("/", { waitUntil: "networkidle" });
  await page.locator('input[type="file"]').first().setInputFiles(accessoryFixture);
  await page.locator("#max-updates").fill("8");
  await page.getByRole("button", { name: "Run Mandrake" }).click();

  await expect(page.getByText("Final embedding", { exact: true })).toBeVisible({ timeout: 150_000 });
  await expect(page.locator(".js-plotly-plot")).toBeVisible();
  expect(errors).toEqual([]);
});

test("HDBSCAN controls and cluster CSV work without browser errors", async ({ page }) => {
  const errors = collectBrowserErrors(page);
  await page.goto("/", { waitUntil: "networkidle" });
  await page.locator("#hdbscan").check();

  const accessory = [
    "Gene\ta\tb\tc\td\te",
    "g1\t1\t1\t0\t0\t1",
    "g2\t1\t0\t1\t0\t0",
    "g3\t0\t1\t1\t1\t0",
    "g4\t0\t0\t1\t1\t1",
    "g5\t1\t0\t0\t1\t1",
  ].join("\n");
  const labels = "a\tgroup-a\nb\tgroup-a\nc\tgroup-b\nd\tgroup-b\ne\tgroup-a\n";
  const fileInputs = page.locator('input[type="file"]');
  await fileInputs.nth(0).setInputFiles({
    name: "hdbscan.Rtab",
    mimeType: "text/plain",
    buffer: Buffer.from(accessory),
  });
  await fileInputs.nth(1).setInputFiles({
    name: "hdbscan.labels.tsv",
    mimeType: "text/tab-separated-values",
    buffer: Buffer.from(labels),
  });
  await page.locator("#max-updates").fill("100");
  await page.getByRole("button", { name: "Run Mandrake" }).click();

  await expect(page.getByText("Final embedding", { exact: true })).toBeVisible();
  await expect(page.locator("#hdbscan-cluster-summary")).toContainText("HDBSCAN");
  await expect(page.locator("#download-clusters")).toBeVisible();
  const toggles = page.locator(".colour-switch button");
  await expect(toggles).toHaveCount(2);
  await toggles.nth(1).click();
  await expect(toggles.nth(1)).toHaveAttribute("aria-pressed", "true");

  const clusterTraceNames = await page.locator(".js-plotly-plot").evaluate((element) =>
    (element as HTMLElement & { data: Array<{ name?: unknown }> }).data.map((trace) => trace.name),
  );
  expect(clusterTraceNames.some((name) => String(name).startsWith("Cluster ") || name === "Noise")).toBe(true);

  const downloadPromise = page.waitForEvent("download");
  await page.locator("#download-clusters").click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toBe("hdbscan.embedding_hdbscan_clusters.csv");
  const stream = await download.createReadStream();
  if (!stream) throw new Error("cluster CSV download did not expose a readable stream");
  const chunks: Buffer[] = [];
  for await (const chunk of stream) chunks.push(Buffer.from(chunk));
  const rows = Buffer.concat(chunks).toString("utf8").trimEnd().split("\n");
  expect(rows).toHaveLength(6);
  expect(rows[0]).toBe("id,hdbscan_cluster__autocolour");
  expect(errors).toEqual([]);
});

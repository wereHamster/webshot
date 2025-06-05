import { chromium, devices } from "npm:playwright@1.49.1";

const port = parseInt(Deno.env.get("PORT") ?? "3000", 10);

Deno.serve({ port }, async (req) => {
  const [_, url] = new URL(req.url).pathname.match(
    "/og/(.*)$",
  )!;

  const browser = await chromium.launch({
    args: ["--no-sandbox", "--disable-dev-shm-usage"],
  });
  const context = await browser.newContext(devices["Desktop Chrome HiDPI"]);
  const page = await context.newPage();
  await page.setViewportSize({
    width: 1200,
    height: 630,
  });
  await page.goto(`https://${url}`, { waitUntil: "networkidle0" });
  const image = (await page.screenshot({
    type: "jpeg",
    quality: 90,
    fullPage: true,
  })) as Uint8Array;

  await browser.close();

  return new Response(image, {
    status: 200,
    headers: {
      "Content-Type": "image/jpeg",
    },
  });
});

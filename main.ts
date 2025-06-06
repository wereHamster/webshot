import { chromium, devices } from "npm:playwright@1.52.0";

const port = parseInt(Deno.env.get("PORT") ?? "3000", 10);

const browserPromise = chromium.launch({
  args: ["--no-sandbox", "--disable-dev-shm-usage"],
});

Deno.serve({ port }, async (req) => {
  const [_, url0] = new URL(req.url).pathname.match("/og/(.*)$")!;

  const url = url0 + "?" + new URL(req.url).searchParams.toString();

  const browser = await browserPromise;

  const context = await browser.newContext(devices["Desktop Chrome"]);
  const page = await context.newPage();
  await page.setViewportSize({
    width: 1200,
    height: 630,
  });

  await page.goto(`https://${url}`, { waitUntil: "networkidle" });

  const image = await page.screenshot({
    type: "jpeg",
    quality: 90,
    fullPage: true,
  });

  await context.close();

  return new Response(image, {
    status: 200,
    headers: {
      "Content-Type": "image/jpeg",
    },
  });
});

import puppeteer from "https://deno.land/x/puppeteer@9.0.1/mod.ts";

const port = parseInt(Deno.env.get("PORT") ?? "3000", 10);
const server = Deno.listen({ port });
console.log(`http://localhost:${port}/`);

for await (const connection of server) {
  handleConnection(connection);
}

async function handleConnection(connection: Deno.Conn) {
  const httpConnection = Deno.serveHttp(connection);

  for await (const requestEvent of httpConnection) {
    const [_, url] = new URL(requestEvent.request.url).pathname.match(
      "/og/(.*)$"
    )!;

    const browser = await puppeteer.launch({
      args: [
        "--no-sandbox",
        "--disable-dev-shm-usage",
      ],
    });
    const page = await browser.newPage();
    await page.setViewport({ width: 1200, height: 630, deviceScaleFactor: 2 });
    await page.goto(`https://${url}`, { waitUntil: "networkidle0" });
    const image = (await page.screenshot({
      type: "png",
      fullPage: true,
    })) as Uint8Array;

    await browser.close();

    requestEvent.respondWith(
      new Response(image, {
        status: 200,
        headers: {
          "Content-Type": "image/png",
        },
      })
    );
  }
}

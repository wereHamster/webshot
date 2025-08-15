import { chromium, devices } from "npm:playwright@1.52.0";

import {
  // biscuit,
  authorizer,
  KeyPair,
  PrivateKey,
  Biscuit,
  AuthorizerBuilder,
} from "@biscuit-auth/biscuit-wasm";

const port = parseInt(Deno.env.get("PORT") ?? "3000", 10);

const browserPromise = chromium.launch({
  args: ["--no-sandbox", "--disable-dev-shm-usage"],
});

Deno.serve({ port }, async (req) => {
  if (
    req.method === "POST" &&
    new URL(req.url).pathname === "/webshot.WebShot/Capture"
  ) {
    const privateKey = PrivateKey.fromString(
      Deno.env.get("BISCUIT_PRIVATE_KEY"),
    );
    const keyPair = KeyPair.fromPrivateKey(privateKey);

    // const builder = biscuit`
    //   operation("Capture")
    // `;
    // console.log(builder.build(privateKey).toBase64());

    const auth = authorizer`
      allow if operation("Capture");
    `;

    const token = Biscuit.fromBase64(
      req.headers.get("Authorization").slice(7),
      keyPair.getPublicKey(),
    );

    const authorizerBuilder = new AuthorizerBuilder();
    authorizerBuilder.merge(auth as any);

    const authz = authorizerBuilder.buildAuthenticated(token);
    authz.authorize();

    const image = await doCapture(await req.json());

    return new Response(image, {
      status: 200,
      headers: {
        "Content-Type": "image/png",
      },
    });
  }

  const match = new URL(req.url).pathname.match("/og/(.*)$");
  if (!match) {
    return new Response("Not Found", { status: 404 });
  }

  const [_, url0] = match;
  if (!url0) {
    return new Response("Bad Request", { status: 400 });
  }

  const url = url0 + "?" + new URL(req.url).searchParams.toString();

  const image = await doCapture({
    device: {
      viewport: { width: 1200, height: 630 },
    },
    input: { kind: "url", value: "https://" + url },
    target: { kind: "page" },
  });

  return new Response(image, {
    status: 200,
    headers: {
      "Content-Type": "image/png",
    },
  });
});

interface CaptureRequest {
  device: {
    viewport: {
      width: number;
      height: number;
    };
    scale?: number;
  };

  input: { kind: "url"; value: string } | { kind: "contents"; value: string };

  target:
    | { kind: "viewport" }
    | { kind: "page" }
    | { kind: "element"; locator: string };
}

async function doCapture(request: CaptureRequest): Promise<Uint8Array> {
  const browser = await browserPromise;

  const context = await browser.newContext({
    ...devices["Desktop Chrome"],
    viewport: request.device.viewport,
    deviceScaleFactor: request.device.scale ?? 1,
  });
  const page = await context.newPage();

  if (request.input.kind === "url") {
    await page.goto(request.input.value, { waitUntil: "networkidle" });
  } else {
    await page.setContent(request.input.value, { waitUntil: "networkidle" });
  }

  const image = await (() => {
    if (request.target.kind === "viewport") {
      return page.screenshot({
        type: "png",
      });
    } else if (request.target.kind === "page") {
      return page.screenshot({
        type: "png",
        fullPage: true,
      });
    } else {
      return page.locator(request.target.locator).screenshot({
        type: "png",
      });
    }
  })();

  await context.close();

  return image;
}

import { Browser, chromium, devices } from "npm:playwright@1.52.0";

import {
  // biscuit,
  authorizer,
  KeyPair,
  PrivateKey,
  Biscuit,
  AuthorizerBuilder,
} from "@biscuit-auth/biscuit-wasm";

/**
 * The port on which the HTTP server listens.
 */
const port: number = parseInt(Deno.env.get("PORT") ?? "3000", 10);

/**
 * Biscuit private and public keys. These are used to sign and verify
 * authorization tokens.
 */
const { privateKey, publicKey } = (() => {
  const privateKeyString = Deno.env.get("BISCUIT_PRIVATE_KEY");
  if (!privateKeyString) {
    throw new Error("BISCUIT_PRIVATE_KEY environment variable is not set");
  }

  const privateKey = PrivateKey.fromString(privateKeyString);

  return {
    privateKey,
    publicKey: KeyPair.fromPrivateKey(privateKey).getPublicKey(),
  };
})();

/*
 * We start the browser as soon as possible, even before the HTTP server
 * starts listening to incoming connections.
 */
const browserPromise: Promise<Browser> = chromium.launch({
  args: ["--no-sandbox", "--disable-dev-shm-usage"],
});

Deno.serve({ port }, async (req) => {
  const url = new URL(req.url);

  const authorization = req.headers.get("Authorization");
  if (!authorization) {
    return new Response("Unauthorized", { status: 401 });
  }

  /*
   * Extract the Biscuit token from the Authorization header.
   */
  let token: Biscuit;
  try {
    token = Biscuit.fromBase64(authorization.slice(7), publicKey);
  } catch (error: unknown) {
    console.log("Biscuit.fromBase64", { authorization }, error);
    return new Response("Bad Request", { status: 400 });
  }

  if (req.method === "POST" && url.pathname === "/v1/render") {
    const auth = authorizer`
      allow if operation("Render");
    `;

    // const builder = biscuit`
    //   operation("Render")
    // `;
    // console.log(builder.build(privateKey).toBase64());

    const authorizerBuilder = new AuthorizerBuilder();
    authorizerBuilder.merge(auth as any);

    const authz = authorizerBuilder.buildAuthenticated(token);
    authz.authorize();

    const image = await doRender(await req.json());

    return new Response(image, {
      status: 200,
      headers: {
        "Content-Type": "image/png",
      },
    });
  }

  if (req.method === "POST" && url.pathname === "/v1/capture") {
    // const builder = biscuit`
    //   operation("Capture")
    // `;
    // console.log(builder.build(privateKey).toBase64());

    const auth = authorizer`
      allow if operation("Capture");
    `;

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

  return new Response("Not Found", { status: 404 });
});

interface RenderRequest {
  device: {
    viewport: {
      width: number;
      height: number;
    };
    scale?: number;
  };

  input: string;
}

async function doRender(request: RenderRequest): Promise<Uint8Array> {
  const browser = await browserPromise;

  const context = await browser.newContext({
    ...devices["Desktop Chrome"],
    viewport: request.device.viewport,
    deviceScaleFactor: request.device.scale ?? 1,
  });

  const page = await context.newPage();

  await page.setContent(request.input, { waitUntil: "load" });

  const image = await page.screenshot({
    type: "png",
  });

  await context.close();

  return image;
}

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

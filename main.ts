import { Browser, chromium, devices } from "npm:playwright@1.52.0";

import {
  // biscuit,
  authorizer,
  Biscuit,
  KeyPair,
  PrivateKey,
  rule,
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
  args: [
    "--no-sandbox",
    "--disable-dev-shm-usage",

    // Attempt to improve font rendering.
    "--font-render-hinting=none",
    "--disable-font-subpixel-positioning",
    "--disable-gpu-sandbox",
    "--force-color-profile=srgb",
    "--disable-background-timer-throttling",
    "--disable-renderer-backgrounding",
  ],
});

// const builder = biscuit`
//   user("nobody");
//
//   check if time($time), $time < ${new Date(Date.now() + 365 * 24 * 60 * 60 * 1000)};
// `;
// console.log(builder.build(privateKey).toBase64());

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
    const renderRequest: RenderRequest = await req.json();

    const auth = authorizer`
      time(${new Date()});
      operation("render");

      allow if user($u);
    `;

    const authz = (auth as any).buildAuthenticated(token);
    try {
      authz.authorize();
    } catch (error: unknown) {
      console.log(error);
      return new Response("Unauthorized", { status: 401 });
    }

    const user = getUser(authz);

    console.log(`Render Request from user:${user}`);

    const image = await doRender(renderRequest);

    return new Response(image, {
      status: 200,
      headers: {
        "Content-Type": "image/png",
      },
    });
  }

  if (req.method === "POST" && url.pathname === "/v1/capture") {
    const captureRequest: CaptureRequest = await req.json();

    const url = new URL(captureRequest.input);

    const auth = authorizer`
      time(${new Date()});
      operation("capture");
      hostname(${url.hostname});

      allow if user($u);
    `;

    const authz = (auth as any).buildAuthenticated(token);
    try {
      authz.authorize();
    } catch (error: unknown) {
      console.log(error);
      return new Response("Unauthorized", { status: 401 });
    }

    const user = getUser(authz);

    console.log(
      `Capture Request from user:${user} for hostname:${url.hostname}`,
    );

    const image = await doCapture(captureRequest);

    return new Response(image, {
      status: 200,
      headers: {
        "Content-Type": "image/png",
      },
    });
  }

  return new Response("Not Found", { status: 404 });
});

function getUser(authz: any) {
  const facts: any[] = authz.queryWithLimits(
    rule`u($user) <- user($user)`,
    {},
  );

  return facts[0].terms()[0];
}

interface RenderRequest {
  device: {
    viewport: {
      width: number;
      height: number;
    };
    scale?: number;
    extraHTTPHeaders?: Record<string, string>;
  };

  input: string;
}

async function doRender(request: RenderRequest): Promise<Uint8Array> {
  const browser = await browserPromise;

  const context = await browser.newContext({
    ...devices["Desktop Chrome"],
    viewport: request.device.viewport,
    deviceScaleFactor: request.device.scale ?? 1,
    extraHTTPHeaders: request.device.extraHTTPHeaders ?? {},
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
    extraHTTPHeaders?: Record<string, string>;
  };

  input: string;

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
    extraHTTPHeaders: request.device.extraHTTPHeaders ?? {},
  });
  const page = await context.newPage();

  await page.goto(request.input, { waitUntil: "networkidle" });

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

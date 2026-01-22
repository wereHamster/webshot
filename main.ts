import { SpanStatusCode, trace } from "@opentelemetry/api";

import { Browser, chromium, devices } from "playwright";

import {
  Authorizer,
  // biscuit,
  authorizer,
  Biscuit,
  KeyPair,
  PrivateKey,
  rule,
} from "@biscuit-auth/biscuit-wasm";

const tracer = trace.getTracer("webshot");

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
    "--disable-gpu",

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
  return await tracer.startActiveSpan("request", async (span) => {
    try {
      const url = new URL(req.url);

      if (url.pathname === "/") {
        return new Response("", { status: 200 });
      }

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
          authz.authorizeWithLimits({
            max_facts: 1000, // default: 1000
            max_iterations: 100, // default: 100
            max_time_micro: 100_000, // default: 1000 (1ms)
          });
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
          tracer.startActiveSpan("authz.authorizeWithLimits", (span) => {
            try {
              return authz.authorizeWithLimits({});
            } finally {
              span.end();
            }
          });
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
    } catch (error: unknown) {
      span.setStatus({
        code: SpanStatusCode.ERROR,
      });

      throw error;
    } finally {
      span.end();
    }
  });
});

/**
 * Extract the user() fact from the Authorizer.
 *
 * All our tokens are expected to have that fact.
 */
function getUser(authz: Authorizer): string {
  const facts = tracer.startActiveSpan("authz.queryWithLimits", (span) => {
    try {
      return authz.queryWithLimits(rule`u($user) <- user($user)`, {});
    } finally {
      span.end();
    }
  });

  const user = facts[0]?.terms()?.[0];
  invariant(!!user, "token must have a user fact");

  return user;
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
  const browser = await tracer.startActiveSpan(
    "launchBrowser",
    async (span) => {
      try {
        return await browserPromise;
      } finally {
        span.end();
      }
    },
  );

  const context = await tracer.startActiveSpan("newContext", async (span) => {
    try {
      return await browser.newContext({
        ...devices["Desktop Chrome"],
        viewport: request.device.viewport,
        deviceScaleFactor: request.device.scale ?? 1,
        extraHTTPHeaders: request.device.extraHTTPHeaders ?? {},
      });
    } finally {
      span.end();
    }
  });

  const page = await tracer.startActiveSpan("newPage", async (span) => {
    try {
      return await context.newPage();
    } finally {
      span.end();
    }
  });

  try {
    await tracer.startActiveSpan("page.goto", async (span) => {
      try {
        await page.goto(request.input, { waitUntil: "networkidle" });
      } finally {
        span.end();
      }
    });

    return await tracer.startActiveSpan("page.screenshot", async (span) => {
      try {
        return await (() => {
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
      } finally {
        span.end();
      }
    });
  } catch (error: unknown) {
    /*
     * If an error occurs, dump the request URL and error to the console.
     *
     * Also, attempt to take a screenshot for debugging purposes. This screenshot
     * is taken with really low quality so that the image is as small as possible.
     * This is because the platform where this service is deployed may limit the
     * size of individual log entries.
     */

    console.error({ url: request.input }, error);

    try {
      const image = await page.screenshot({ type: "jpeg", quality: 10 });
      console.info(image.toString("base64"));
    } catch {
      // ignore
    }

    throw error;
  } finally {
    await tracer.startActiveSpan("cleanup", async (span) => {
      try {
        await page.close();
        await context.close();
      } finally {
        span.end();
      }
    });
  }
}

function invariant<const T = unknown>(
  condition: T,
  message: string,
): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

import { assertEquals, assertExists } from "jsr:@std/assert";
import { biscuit, PrivateKey } from "@biscuit-auth/biscuit-wasm";

const BASE_URL = "http://localhost:3000";
const build = Deno.env.get("BUILD") ?? "head";

function generateTestToken(): string {
  const privateKeyString = Deno.env.get("BISCUIT_PRIVATE_KEY");
  if (!privateKeyString) {
    throw new Error("BISCUIT_PRIVATE_KEY environment variable is not set");
  }

  const privateKey = PrivateKey.fromString(privateKeyString);

  const builder = biscuit`
    user("nobody");

    check if time($time), $time < ${new Date(Date.now() + 60 * 60 * 1000)};
  `;

  return builder.build(privateKey).toBase64();
}

interface UploadImageRequest {
  build: string;
  collection: string;
  snapshot: string;
  formula: string;
  payload: File;
}

async function uploadImage(
  { build, collection, snapshot, formula, payload }: UploadImageRequest,
) {
  const body = new FormData();
  body.set("collection", collection);
  body.set("snapshot", snapshot);
  body.set("formula", formula);
  body.set("payload", payload);

  const res = await fetch(
    `https://app.urnerys.dev/api/v1/projects/webshot/builds/${build}/images`,
    {
      method: "POST",
      body,
    },
  );

  if (!res.ok) {
    console.log(res.statusText);
    throw res;
  }

  await res.text();
}

Deno.test("render", async () => {
  const payload = {
    device: {
      viewport: {
        width: 1200,
        height: 600,
      },
      scale: 2,
    },
    input: "<h1 style='color:red;'>Hello World",
  };

  const token = generateTestToken();

  const response = await fetch(`${BASE_URL}/v1/render`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": `Bearer ${token}`,
    },
    body: JSON.stringify(payload),
  });

  assertEquals(response.status, 200);
  assertExists(response.body);

  const contentType = response.headers.get("content-type");
  assertEquals(contentType, "image/png");

  const imageData = new Uint8Array(await response.arrayBuffer());
  assertExists(imageData);
  assertEquals(imageData.length > 0, true);

  await uploadImage({
    build,
    collection: "End-to-End Tests/v1",
    snapshot: "Render",
    formula: "1200x600-scale:2",
    payload: new File([imageData], "image.png", { type: "image/png" }),
  });
});

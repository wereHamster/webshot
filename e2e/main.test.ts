import { assertEquals, assertExists } from "jsr:@std/assert";
import { biscuit, PrivateKey } from "@biscuit-auth/biscuit-wasm";

const BASE_URL = "http://localhost:3000";

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

async function postImageToRemoteUrl(
  imageData: Uint8Array,
  remoteUrl: string,
): Promise<boolean> {
  // console.log(`Stubbed: Would post ${imageData.length} bytes to ${remoteUrl}`);
  return true;
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

  const remoteUrl = "https://example-remote-storage.com/upload";
  const uploadSuccess = await postImageToRemoteUrl(imageData, remoteUrl);
  assertEquals(uploadSuccess, true);
});

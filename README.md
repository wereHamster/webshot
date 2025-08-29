<div align="center">
  <h1>WebShot</h1>
  <div>Turn <b>WEB</b>-sites into screen-<b>SHOT</b>-s</div>
</div>

---

WebShot is a service that converts HTML (supplied either via direct input or via URL) into images.

Its API is intentionally constrained to only cover a narrow set of use cases.
It is not a general "turn any website into image" service, and does not expose the full capabilitis of the underlying HTML rendering engine (Chrome, via Playwright).

It was built primarily to support "image export" features that are built into certain types of websites.
Some example use cases are:

- Generate dynamic Open Graph images.
- Allow users to download dynamically generated images.

## API

The service was initially designed with gRPC in mind.
However, writing gRPC servers in JavaScript (Deno runtime, specifically) is not supported well.
Therefore the current API is based on plain HTTP.
The API endpoints and request / response types are designed to be compatible with standard HTTP/JSON-to-gRPC transcoding semantics.

### Authorization

Authorization is done via Bearer tokens using [Biscuit](https://www.biscuitsec.org/) - a cryptographically secured authorization token format.

#### Configuring your Private Key

Ensure your runtime environment defines an environment variable `BISCUIT_PRIVATE_KEY` containing your private key. **Note:** The key string must be prefixed by the encryption algorithm, e.g.

```
BISCUIT_PRIVATE_KEY=ed25519-private/my-super-secret-key
```

#### Token Format

Tokens are passed in the `Authorization` header as Bearer tokens.

```
Authorization: Bearer <base64-encoded-biscuit-token>
```

#### Required Claims

All tokens must contain a `user` fact identifying the authenticated user.

There are no implicit permissions associated with a user.
The user fact is currently only used for logging.
This is due to the stateless nature of the service (there is no database to store user permissions in).

```
user("username");
```

#### Operation-Specific Authorization

Each endpoint enforces specific authorization rules.

**For `/v1/render`:**
- Requires `user($u)` fact (any authenticated user)
- Operation is automatically tagged as `operation("render")`

**For `/v1/capture`:**
- Requires `user($u)` fact (any authenticated user)
- Operation is automatically tagged as `operation("capture")`
- Hostname is automatically extracted and tagged as `hostname("example.com")`

#### Creating Tokens

Tokens must be signed with the service's private key.
Here's an example of creating a basic token:

```typescript
import { biscuit, PrivateKey } from "@biscuit-auth/biscuit-wasm";

const privateKey = PrivateKey.fromString("your-private-key");

const builder = biscuit`
  user("alice");
`;

const token = builder.build(privateKey).toBase64();
```

#### Attenuated Tokens

You can create more restrictive tokens by adding additional constraints.
For example, to create a token that can only be used for capture operations on `example.com`:

```typescript
import { biscuit, PrivateKey } from "@biscuit-auth/biscuit-wasm";

const privateKey = PrivateKey.fromString("your-private-key");

const builder = biscuit`
  user("alice");

  check if time($time), $time < ${new Date("2026-01-01T00:00:00Z")};
  check if operation("capture");
  check if hostname("example.com");

`;

const restrictedToken = builder.build(privateKey).toBase64();
```

This token will be rejected if used for:
- The `/v1/render` endpoint (wrong operation)
- Capturing any hostname other than `example.com`
- Any request made after the specified expiration date

The service validates tokens using the corresponding public key and enforces the authorization rules at request time.

#### Security Model

- Tokens are cryptographically signed and cannot be forged
- Each request is authorized based on the user identity and the specific operation being performed
- For capture requests, the target hostname is automatically validated as part of the authorization check
- Time-based validation ensures tokens are evaluated with the current timestamp

### Endpoints

#### `/v1/render`

Render static HTML into image.

You supply the HTML, set the browser viewport size, and get an image back.

#### `/v1/capture`

Capure an image of a URL.

You give and URL, along with the browser viewport size, and what area should be captured. You get an image back.

## Performance

Expect a latency of around one to three seconds to render or capture a simple, static HTML page.
If the page needs to load external resources (images, fonts, data via XHR etc.), the latency will increase accordingly.

## Security

The service is not designed to be directly accessible by end-users.
Even though the API exposes only a limited set of capabilities, they could still be used by malicious users to exfiltrate information via the created screenshots.
You should only give API access to users who you fully trust.

## Image Stabilization

One of the big challenges when taking images of a HTML page is how to wait until the page has finished rendering and has stabilized enough so that the image can be taken.
For example, images and fonts are external resources that need to be fetched asynchronously.
The page may need to run JavaScript to fetch data from remote APIs and dynamically render certain parts of the page.
Or the page uses CSS transitions that need to settle.

One commonly used strategy is to wait until there are no more network requests for a certain amount of time (usually 500ms).
That strategy works surprisingly well for most cases, but comes with an inherent extra delay.
If the page itself renders quickly (think inline HTML, no JavaScript, no CSS transitions, etc) this `networkidle` delay itself is more costly than the actual rendering.

It is difficult to come up with alternative strategies that do not involve a fixed delay but work well for a broad range of situations.
The best option for WebShot is to offer multiple strategies and let the user pick one.

At this point however, WebShot does not offer a way to customize the image stabilization strategy.
It uses `networkidle`, with its inherent 500ms delay.

Here are some useful resources about what strategies other products have taken:

 - GitHub: https://github.blog/open-source/git/framework-building-open-graph-images/
 - Argos CI: TODO

> Self-hosted service to convert HTML into an image

WebShot is a service that exposes a HTTP+JSON and GRPC API to convert some HTML (supplied either directly or via URL) to a JPEG image.

The API is intentionally limited to only cover a narrow set of use cases.
It does not expose the full capabilities of the underlying HTTP rendering engine (Chrome, via Playwright).

Some example use cases are:

 - Generate dynamic open graph images.
 - Allow users to download dynamically generated images.

The service is currently built on top of a headless Chrome browser.
It comes with a few constraints and trade-offs.
For example, you have to assume a latency of about 2-5 seconds for each image that is generated.

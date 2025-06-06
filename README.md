> Self-hosted service to convert HTML into an image

WebShot is a service that exposes a HTTP+JSON and GRPC API to convert some HTML (supplied either directly or via URL) to a JPEG image.

The API is intentionally limited to only cover a narrow set of use cases:
It does not expose the full capabilities of the underlying HTTP rendering engine (Chrome, via Playwright).

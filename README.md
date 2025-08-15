<div align="center">
  <h1>WebShot</h1>
  <div>turn <b>web</b>sites into screen<b>shot</b>s</div>
</div>

---

WebShot is a service that converts HTML (supplied either via direct input or via URL) into images.

Its API is intentionally constrained to only cover a narrow set of use cases.
It is not a general "turn any website into image" service, and does not expose the full capabilitis of the underlying HTML rendering engine (Chrome, via Playwright).

It was built primarily to support "image export" features that are built into some types of websites. Some example use cases are:

- Generate dynamic Open Graph images.
- Allow users to download dynamically generated images.

## API

### /v1/render

Render static HTML into image.

You supply the HTML, set the browser viewport size, and get an image back.

### /v1/capture

Capure an image of a URL.

You give and URL, along with the browser viewport size, and what area should be captured. You get an image back.

## Performance

Expect a latency of around one to three seconds to render or capture a simple, static HTML page.
If the page needs to load external resources (images, fonts, data via XHR etc.), the latency will increase accordingly.

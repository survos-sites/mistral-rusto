# mistral-rusto

Deploy configuration for our hosted Mistral-compatible OCR endpoint, https://mistral-rusto.survos.com.

The server itself is `rusto-server` in [byrizki/rusto-rs](https://github.com/byrizki/rusto-rs) (`crates/rusto-server`),
merged in [#31](https://github.com/byrizki/rusto-rs/pull/31) after the proposal in
[#28](https://github.com/byrizki/rusto-rs/issues/28). It started as a prototype in this repository; the code now lives
upstream and this repository only holds what is specific to our deployment:

- `Dockerfile`: builds `rusto-server` from rusto-rs at a pinned revision, downloads the `tiny` and `small` PP-OCRv6
  models from a pinned Hugging Face revision, verifies them against `models.sha256`, runs rusto-server's tests against
  them, and sets our defaults (`OCR_AUTOROTATE=true`, pool size 1, default model `tiny`).
- `app.json`: Dokku health check on `/health`.
- `docs/dokku.md`: how it is deployed.

For what the server supports, its environment variables and how to run it, see
[crates/rusto-server/README.md](https://github.com/byrizki/rusto-rs/blob/main/crates/rusto-server/README.md).

To move to a newer rusto-rs, change `RUSTO_REV` in the `Dockerfile` and push to Dokku; the build runs the tests.

Earlier measurements of `tiny` against `small` on gravestone photos: [byrizki/rusto-rs#29](https://github.com/byrizki/rusto-rs/issues/29).

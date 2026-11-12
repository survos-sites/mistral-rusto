# mistral-rusto

A small HTTP service that answers Mistral's `POST /v1/ocr` request shape, using local OCR from
[rusto-rs](https://github.com/byrizki/rusto-rs) (PP-OCRv6 `tiny` and `small`, pure-Rust inference). It exists so
code written against Mistral's OCR API can be pointed at a self-hosted server by changing the base URL.

Status: prototype, in use for our own tests. It is a stop-gap: the proposal to build this into rusto-rs itself is
[byrizki/rusto-rs#28](https://github.com/byrizki/rusto-rs/issues/28), and if that happens this repo can go away.

## What it does and does not do

**Supported**
- `POST /v1/ocr` with `document.type` `image_url` or `document_url`, as a `data:` URI (base64) or, for hosts you
  allow, an `http(s)` URL. The URL may be a plain string or `{"url": "..."}`. PNG, JPEG, WebP and TIFF.
- Response: `pages[]` with `index`, `markdown`, `images` (always empty) and `dimensions` (`dpi` is null), plus `model` and
  `usage_info` (`pages_processed`, `doc_size_bytes`). Each page also has a non-Mistral `lines` array
  (`text`, `score`, `box`) that Mistral clients ignore.
- `GET /v1/models`, `GET /health`, optional bearer-token auth, Mistral's error envelope.
- Choosing the model per request: `"model": "tiny"` or `"small"` (also `rusto-ppocrv6-small`).
  Mistral ids (`mistral-ocr-*`) and no model use the default, `tiny`; any other name is a 400.

**Not supported yet:** PDFs (rejected), layout blocks and bounding boxes, tables, headers and footers, image
extraction, confidence-score options, annotations, `pages`, and the `/v1/files` upload flow. `markdown` is just the
recognized lines in rusto's output order. These need layout output from rusto-rs; see the upstream issue.

**Compatibility is untested against Mistral's official SDKs.** We call it with plain HTTP requests shaped like Mistral's
(from PHP and curl). Do not assume more than the list above.

## Measurements

Tiny against Small, 87 phone photos of gravestones (28 with a Mistral transcript as reference), warm, over HTTP:

| | tiny | small |
|---|---|---|
| mean time | 349 ms | 539 ms |
| word recall vs Mistral (28 photos) | 0.60 | 0.705 |

Details and caveats: [byrizki/rusto-rs#29](https://github.com/byrizki/rusto-rs/issues/29). Against figmark (Docling, a CLI
that loads its models on every run), this service was roughly 50x faster on three images, which is not a like-for-like
comparison; Docling returns headings and tables that this service does not.

## Run

Model weights are not in the repo. Put one directory per model under a models directory, each with `det.onnx`,
`rec.onnx` and `dict.txt` (PP-OCRv6 weights from [byrizki/rusto-models](https://huggingface.co/byrizki/rusto-models)):

```bash
OCR_MODELS_DIR=/path/to/models cargo run --release     # models/tiny, models/small
```

| Env | Default | |
|---|---|---|
| `OCR_MODELS_DIR` | | one subdirectory per model; each loads on first use |
| `OCR_MODEL_DIR` | `models/tiny` | single-model shortcut, served as `tiny` (used when `OCR_MODELS_DIR` is unset) |
| `OCR_DEFAULT_MODEL` | `tiny` | |
| `OCR_LISTEN` | `127.0.0.1:8895` | |
| `OCR_POOL_SIZE` | `2` | engines per model; an engine is single-threaded |
| `OCR_API_KEY` | unset | if set, `/v1/*` requires `Authorization: Bearer <key>` |
| `OCR_ALLOWED_HOSTS` | empty | hosts URLs may be fetched from (no redirects); `data:` URIs always work |

```bash
curl -s localhost:8895/v1/ocr -H 'content-type: application/json' -d '{
  "model": "small",
  "document": {"type": "image_url", "image_url": "data:image/png;base64,..."}
}'
```

## Docker and deploy

The `Dockerfile` builds the binary, downloads `tiny` and `small` from a pinned Hugging Face revision, verifies them
against `models.sha256`, and runs the tests inside the build. The image is about 146 MB and used about 231 MiB of RAM
with both models loaded. Dokku notes: [docs/dokku.md](docs/dokku.md).

## Test

```bash
OCR_MODELS_DIR=/path/to/models cargo test
```

## Licence

Not chosen yet. rusto-rs is MIT and the PP-OCRv6 models come from PaddleOCR (Apache-2.0).

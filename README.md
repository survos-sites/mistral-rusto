# ocr-api

Mistral-compatible `POST /v1/ocr` on top of [rusto-rs](https://github.com/byrizki/rusto-rs)
(PP-OCRv6 Tiny, pinned at f988c44). Proposal upstream: byrizki/rusto-rs#28.

**v1 is lines-only.** `pages[].markdown` is the recognized lines in rusto's output order.
No blocks, tables or images yet (waiting on upstream). Each page also has a non-Mistral
`lines` array (`text`, `score`, `box`) that Mistral clients ignore. PDFs are rejected.

## Speed

Measured 2026-11-12 on an M4, same three images, warm, through the ai-pipeline-demo `/ocr` page:

| Image | ocr-api (rusto Tiny) | figmark CLI (Docling) |
|---|---|---|
| `HELLO OCR 123`, 700x160 | 0.39 s, correct | 13.5 s, empty |
| photographed invoice | 0.22 s, 239 chars | 10.8 s, 135 chars |
| clean typed invoice | 0.23 s, 582 chars | 12.0 s, 2,069 chars, with a markdown table |

That is roughly 50x faster. The comparison is not like-for-like: figmark has no server, so every
figmark number includes starting Python and loading its models, while ocr-api keeps its engines
resident. Docling does return structure (headings, tables) that ocr-api does not yet; ocr-api
returns plain lines. Neither has been scored on newspaper scans.

## Run

```bash
OCR_MODEL_DIR=/path/to/tiny cargo run --release   # needs det.onnx, rec.onnx, dict.txt
```

| Env | Default | |
|---|---|---|
| `OCR_MODEL_DIR` | `models/tiny` | Tiny weights (same files imagor uses; sha256s in imagor `docker/imagor/ocr-models.sha256`) |
| `OCR_LISTEN` | `127.0.0.1:8895` | |
| `OCR_POOL_SIZE` | `2` | engines; an engine is single-threaded |
| `OCR_API_KEY` | unset | if set, requires `Authorization: Bearer <key>` |
| `OCR_ALLOWED_HOSTS` | empty | hosts `image_url`/`document_url` may be fetched from; `data:` URIs always work |

## Call

```bash
curl -s localhost:8895/v1/ocr -H 'content-type: application/json' -d '{
  "model": "mistral-ocr-latest",
  "document": {"type": "image_url", "image_url": "data:image/png;base64,..."}
}'
```

## Test

```bash
OCR_MODEL_DIR=/path/to/tiny cargo test
```

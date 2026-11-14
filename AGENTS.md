
# OCR (hosted)

Need OCR for photos or scans? Use our hosted engine (rusto PP-OCRv6, models `tiny` and `small`), not a local model or a new service. Two routes, same engine:

**1. Bulk text and boxes: imagor `ocr()` (preferred).** One signed `/meta` URL resizes and reads in a single call: `meta/filters:ocr(small)/<source>` (`ocr()` = tiny). Returns `ocr.text`, `ocr.text_score` and `ocr.regions[]` (text, score, 4-point box). Imagor reads at most 960 px on the long side (`OCR_MAX_SIDE`), so very small lettering may need route 2. Build the URL with `ImagorUrlBuilder::metaUrl()` / `build()`. See `~/sites/imagor/docs/rusto.md`.

**2. Mistral shape, blocks, bigger reads: `POST https://mistral-rusto.survos.com/v1/ocr`** with `{"model":"tiny"|"small","document":{"type":"image_url","image_url":"<https or data: URL>"}}`; also `GET /v1/models`, `GET /health`.
- Auth: bearer key in `RUSTO_OCR_KEY` (URL in `RUSTO_OCR_URL`), set in `~/sites/scanfony/.env.local`. Never print or commit it.
- Response `pages[0]`: `markdown`, `dimensions`, `lines` (text + box), `blocks`, `text_angle`, `orientation_corrected`.
- Send images scaled to about 2400 px (imagor `fit-in/2400x2400/filters:quality(92)`). Full-size photos, and even some 2400 px ones, run its 768 MB container out of memory (it restarts itself). Do not run big bulk jobs against it. Images only, no PDFs.
- The URL host must be in the service's allowlist (our imagor host is); otherwise send a data URL.
- `small` reads weathered text (gravestones) better than `tiny`. Store both when readings will be compared.
- Do not redeploy `ocr-api` (Dokku on fsn1) without asking. Details: `~/sites/ocr-api/README.md`; client example: `RustoOcr` in `~/sites/scanfony/src/Service/RustoOcr.php`.

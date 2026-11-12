# Deploying ocr-api to Dokku (fsn1)

Dockerfile app, built on the server by `git push`. Deployed 2026-11-12 as Dokku app `ocr-api` at https://mistral-rusto.survos.com
(`ocr.survos.com` was left free on purpose).

## Once

```bash
ssh -o BatchMode=yes fsn1 apps:create ocr-api
ssh -o BatchMode=yes fsn1 ports:set ocr-api http:80:8895 https:443:8895
ssh -o BatchMode=yes fsn1 domains:set ocr-api mistral-rusto.survos.com        # Cloudflare A record to 46.225.149.37, proxy OFF (grey cloud); the proxied *.survos.com wildcard breaks Let's Encrypt
ssh -o BatchMode=yes fsn1 resource:limit --memory 768m ocr-api       # measured 231 MiB with tiny + small loaded, one engine each
ssh -o BatchMode=yes fsn1 config:set --no-restart ocr-api OCR_API_KEY="$(openssl rand -hex 32)"
ssh -o BatchMode=yes fsn1 letsencrypt:set ocr-api email <contact email>
ssh -o BatchMode=yes fsn1 letsencrypt:enable ocr-api

git remote add dokku dokku@fsn1.survos.com:ocr-api
git push dokku master
```

The key is generated on your machine and sent over ssh (it is kept in `~/.config/ocr-api.key.env`, mode 600; silence `config:set`, it echoes the value); read it back with
`ssh fsn1 config:get ocr-api OCR_API_KEY` (prefer that to `config:show`, which prints every secret).
Callers send `Authorization: Bearer <key>`. `/health` is open for Dokku's check (`app.json`).

## Settings

| Config | Value | |
|---|---|---|
| `OCR_API_KEY` | random, required in prod | all `/v1/*` routes |
| `OCR_POOL_SIZE` | `1` (image default) | engines per model; each is single-threaded |
| `OCR_ALLOWED_HOSTS` | unset | leave unset: prod accepts uploaded `data:` images only and never fetches URLs |
| `OCR_DEFAULT_MODEL` | `tiny` (image default) | |

Models are baked into the image (tiny 6 MB, small 30 MB) from a pinned Hugging Face revision and checked
against `models.sha256` during the build, and the test suite runs inside the build.

## Callers

- scanfony: `RUSTO_OCR_URL=https://mistral-rusto.survos.com` and `RUSTO_OCR_KEY=<key>` in `.env.local` / the app's config.
- Anything speaking Mistral's `/v1/ocr`: change the base URL and send the key as the API key.

## Check

```bash
curl -s https://mistral-rusto.survos.com/health
curl -s -H "Authorization: Bearer $KEY" https://mistral-rusto.survos.com/v1/models
```

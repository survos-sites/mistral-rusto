# ocr-api: Mistral-compatible POST /v1/ocr on rusto-rs (PP-OCRv6 tiny + small).
# The rusto-rs pin lives in Cargo.toml/Cargo.lock; model weights are pinned by Hugging Face revision
# and verified against models.sha256 (tiny: imagor's pin; small: hashed from the files we evaluated).

FROM rust:1.98.1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
RUN cargo build --release --locked

FROM debian:bookworm-slim AS models
ARG HF_REV=e5c541c590cb88a1631769ce071cd3df31953c23
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /models
RUN for m in tiny small; do mkdir -p $m; for f in det.onnx rec.onnx dict.txt; do \
      curl -fsSL "https://huggingface.co/byrizki/rusto-models/resolve/${HF_REV}/ppocrv6-$m/$f" -o "$m/$f"; done; done
COPY models.sha256 /models.sha256
RUN sha256sum -c /models.sha256

# Tests run against the verified models and fail the build if they break.
FROM build AS test
COPY --from=models /models /models
COPY tests tests
RUN OCR_MODELS_DIR=/models cargo test --release --locked

FROM debian:bookworm-slim AS runtime
RUN useradd --system --uid 10001 ocr
COPY --from=build /src/target/release/ocr-api /usr/local/bin/ocr-api
COPY --from=models /models /opt/ocr/models
# Build waits for the test stage so a failing test blocks the image.
COPY --from=test /src/Cargo.toml /tmp/.tested
ENV OCR_MODELS_DIR=/opt/ocr/models OCR_LISTEN=0.0.0.0:8895 OCR_POOL_SIZE=1 OCR_DEFAULT_MODEL=tiny
USER ocr
EXPOSE 8895
CMD ["ocr-api"]

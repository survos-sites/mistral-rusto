# mistral-rusto deploy: builds rusto-server from byrizki/rusto-rs (merged in PR #31) at a pinned revision.
# Model weights are pinned by Hugging Face revision and verified against models.sha256
# (tiny: imagor's pin; small: hashed from the files we evaluated).

FROM rust:1.98.1-bookworm AS build
# Merge of PR #31 into rusto-rs main.
ARG RUSTO_REV=351106935246d1ee7a607597148794f7d3bd223b
RUN git clone https://github.com/byrizki/rusto-rs.git /src && cd /src && git checkout "$RUSTO_REV"
WORKDIR /src
RUN cargo build --release --locked -p rusto-server

FROM debian:bookworm-slim AS models
ARG HF_REV=e5c541c590cb88a1631769ce071cd3df31953c23
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /models
RUN for m in tiny small; do mkdir -p $m; for f in det.onnx rec.onnx dict.txt; do \
      curl -fsSL "https://huggingface.co/byrizki/rusto-models/resolve/${HF_REV}/ppocrv6-$m/$f" -o "$m/$f"; done; done
COPY models.sha256 /models.sha256
RUN sha256sum -c /models.sha256

# rusto-server's own tests run against the verified models and fail the build if they break. The workspace
# release profile aborts on panic, which the test harness cannot use, hence release-with-debug.
FROM build AS test
COPY --from=models /models /models
RUN OCR_MODELS_DIR=/models cargo test --locked -p rusto-server --profile release-with-debug

FROM debian:bookworm-slim AS runtime
RUN useradd --system --uid 10001 ocr
COPY --from=build /src/target/release/rusto-server /usr/local/bin/rusto-server
COPY --from=models /models /opt/ocr/models
# Build waits for the test stage so a failing test blocks the image.
COPY --from=test /src/Cargo.toml /tmp/.tested
# OCR_AUTOROTATE: our clients send phone photos, so apply EXIF orientation unless a request says otherwise
# (rusto-server's own default is off).
ENV OCR_MODELS_DIR=/opt/ocr/models OCR_LISTEN=0.0.0.0:8895 OCR_POOL_SIZE=1 OCR_DEFAULT_MODEL=tiny OCR_AUTOROTATE=true
USER ocr
EXPOSE 8895
CMD ["rusto-server"]

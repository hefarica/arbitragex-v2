# e2e fixtures sidecar (CI-02 fix, 2026-09-24): compose.hotpath-test.yml
# referenced this Dockerfile but it never existed — docker compose up on the
# hotpath test stack failed at build. Minimal fixture-carrier image: the test
# fixtures ride a volume; this image only provides a shell entrypoint.
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /fixtures
CMD ["sleep", "infinity"]

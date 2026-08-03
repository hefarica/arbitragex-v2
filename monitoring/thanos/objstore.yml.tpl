# Thanos object-store config TEMPLATE — rendered at deploy time.
#
# CI (auto-deploy-vps.yml) renders monitoring/thanos/objstore.yml from this
# template using envsubst with an EXPLICIT allowlist (MINIO_ROOT_USER,
# MINIO_ROOT_PASSWORD), sourced from the gitignored VPS .env. The rendered
# objstore.yml is GITIGNORED — real credentials never enter version control
# (§33 no-secrets-in-versioned-files, arbx-no-hardcode-doctrine).
#
# Thanos reads objstore.yml at runtime; it does NOT expand ${VAR} itself, so
# the template MUST be rendered before the thanos containers start. The render
# step runs after `git reset --hard` and before `docker compose up`, so a render
# failure aborts the deploy with the running stack still intact (R8 fail-honest).
#
# LOCAL DEV: render manually before `docker compose -f docker/compose.dev.yml
# up thanos-sidecar` (compose.dev.yml mounts the RENDERED path). Export the two
# MINIO env vars, then run envsubst with BOTH as the explicit allowlist over
# objstore.yml.tpl -> objstore.yml. (The var names are intentionally NOT written
# with a $ prefix here, because envsubst would otherwise expand them in this
# comment too.) The exact command is in auto-deploy-vps.yml step [2.5/9].
#
# Bucket `arbx-metrics` must exist in Minio before the Thanos sidecar starts
# (the deploy creates it via a one-shot minio/mc container; see THANOS_SETUP.md).

type: S3
config:
  bucket: arbx-metrics
  endpoint: minio:9000
  access_key: ${MINIO_ROOT_USER}
  secret_key: ${MINIO_ROOT_PASSWORD}
  insecure: true
  # Thanos stores blocks as multipart objects; these are safe defaults.
  part_size: 134217728  # 128 MiB

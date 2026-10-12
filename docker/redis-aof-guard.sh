#!/bin/sh
# ============================================================================
# ARBX-AOF-GUARD-01 — pre-boot AOF integrity guard for redis (2026-10-12)
#
# Incident that birthed this gate: 2026-10-12 ~00:43 a stack event interrupted
# an AOF rewrite mid-flight (evidence: temp-appendonly.aof.manifest + empty
# appendonly.aof.11379.incr.aof, both mtime 00:43) leaving
# appendonly.aof.11378.incr.aof truncated. Redis crash-looped on
# "Bad file format reading the append only file" (RestartCount 9, exponential
# backoff) → every service with depends_on redis:service_healthy stalled;
# api-server sat in Created → origin-wide 500s for ~3h until manual
# redis-check-aof --fix. Second AOF corruption of the month (first:
# 2026-09-23 disk-full, appendonly.aof.4975.incr.aof.bak-diskfull-20260923).
#
# Contract (FAIL-OPEN by design): this guard must NEVER block redis boot on
# its own failure. It exits 0 even on internal errors, logging loudly.
# Best case it repairs a truncated tail (redis-check-aof --fix keeps every
# complete record up to the corruption point). Worst case redis behaves
# exactly as it would without the guard. The operator-visible trace of any
# repair is the *.bak-guard-<epoch> file + this container's log.
#
# Runs as one-shot compose init service `redis-aof-guard` before redis
# (depends_on: service_completed_successfully) in compose.prod.yml and
# compose.dev.yml. Host sysctl prerequisite (redis boot warning, fix applied
# 2026-10-12 on the VPS): vm.overcommit_memory=1 persisted in
# /etc/sysctl.d/99-arbx-redis.conf.
# ============================================================================
set -u

DIR=/data/appendonlydir

log() { echo "[redis-aof-guard] $(date -u +%Y-%m-%dT%H:%M:%SZ) $*"; }

if [ ! -d "$DIR" ]; then
  log "no $DIR — fresh volume, nothing to guard"
  exit 0
fi

# Housekeeping: guard backups older than 7 days are deleted (the 2026-09-23
# .bak-diskfull file alone was 90MB; unbounded .bak growth is its own disk
# incident). Failures here are non-fatal by design.
find "$DIR" -name '*.bak-guard-*' -mtime +7 -delete 2>/dev/null || true

FIXED=0
# Multi-part AOF: every incremental part is validated; a truncated tail is
# backed up and repaired to the last complete record.
for f in "$DIR"/appendonly.aof.*.incr.aof; do
  [ -e "$f" ] || continue
  if redis-check-aof "$f" >/dev/null 2>&1; then
    continue
  fi
  log "CORRUPT: $f — backing up and truncating to last valid record"
  if ! cp "$f" "$f.bak-guard-$(date +%s)" 2>/dev/null; then
    log "WARN: backup copy failed for $f (fixing anyway)"
  fi
  if echo y | redis-check-aof --fix "$f" >/dev/null 2>&1; then
    FIXED=$((FIXED + 1))
  else
    log "ERROR: redis-check-aof --fix failed on $f — redis will attempt boot anyway"
  fi
done

# Base parts are RDB snapshots: no --fix tool exists for RDB. Check-only pass
# so a corrupt base is at least LOUD in the log instead of a silent mystery
# crash-loop (recovery for a corrupt base is operator: restore from backup).
for b in "$DIR"/appendonly.aof.*.base.rdb; do
  [ -e "$b" ] || continue
  if ! redis-check-rdb "$b" >/dev/null 2>&1; then
    log "CORRUPT-BASE: $b — RDB cannot be auto-repaired; restore from backup"
  fi
done

if [ "$FIXED" -gt 0 ]; then
  log "repaired $FIXED AOF file(s) — redis can boot now"
else
  log "all AOF incr files OK"
fi
exit 0

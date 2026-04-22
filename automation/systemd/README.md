# ArbitrageX v2 — systemd units

These units run on the host VPS (not inside the docker stack). They orchestrate
backups, restore rehearsal, and — in later PRs — vault-agent + CF tunnel boot.

## Install

```bash
sudo cp automation/systemd/arbx-backup.service /etc/systemd/system/
sudo cp automation/systemd/arbx-backup.timer   /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now arbx-backup.timer
systemctl list-timers | grep arbx-backup
```

## Environment file

`arbx-backup.service` reads `/etc/arbx/backup.env` which is rendered by
vault-agent. The operator's out-of-band setup (phase 5 onboarding) writes the
Vault paths; vault-agent renders them into `/etc/arbx/backup.env` with mode
`0600`.

Expected variables (see `automation/scripts/backup-pg.sh` +
`backup-offsite.sh` for full contract):

```
BACKUP_DIR=/var/backups/arbx
AGE_RECIPIENT=age1qyz...                                  # operator public key
DATABASE_READONLY_URL=postgres://arbx_ro:...@127.0.0.1:5432/arbitragex
RETAIN_DAYS=14
RCLONE_REMOTE=arbx-b2
RCLONE_BUCKET=arbx-backups/prod
```

No provider defaults. If any variable is missing, the script refuses to run —
no-hardcode doctrine.

## Manual dry run

```bash
sudo systemctl start arbx-backup.service
journalctl -u arbx-backup.service -n 200 --no-pager
ls -lh /var/backups/arbx/
rclone ls arbx-b2:arbx-backups/prod | tail -5
```

## Uninstall

```bash
sudo systemctl disable --now arbx-backup.timer
sudo rm /etc/systemd/system/arbx-backup.{service,timer}
sudo systemctl daemon-reload
```

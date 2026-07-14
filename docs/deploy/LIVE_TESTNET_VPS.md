# LIVE_TESTNET VPS Deploy

## Steps
```bash
ssh arbx
cd /opt/arbitragex-v2
git pull
docker compose up -d
curl http://localhost:8080/api/v1/readiness/decision
```

## Verify
- Mode: LIVE_TESTNET
- Mainnet blocked: true

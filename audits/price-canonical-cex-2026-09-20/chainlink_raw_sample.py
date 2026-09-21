import json, urllib.request, time

RPC = "https://ethereum-rpc.publicnode.com"
FEEDS = {
    "ETH/USD": "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419",
    "BTC/USD": "0xF4030086522a5bEEa4988F8cA5B36dbC97BeE88c",
    "USDC/USD": "0x8fFfFfd4AfB6115b954Bd326cbe7B4BA576818f6",
    "USDT/USD": "0x3E7d1eAB13ad0104d2750B8863b489D65364e32D",
    "DAI/USD": "0xAed0c38402a5d19df6E4c03F4E2DceD6e29c1ee9",
}

def latest_round(addr, tries=3):
    import time as _t
    body = json.dumps({
        "jsonrpc": "2.0", "id": 1, "method": "eth_call",
        "params": [{"to": addr, "data": "0xfeaf968c"}, "latest"],
    }).encode()
    last = None
    for attempt in range(tries):
        req = urllib.request.Request(RPC, data=body, headers={
            "Content-Type": "application/json",
            "User-Agent": "arbx-price-audit/1.0",
        })
        try:
            raw = json.load(urllib.request.urlopen(req, timeout=20))
        except Exception as e:
            last = {"http_err": str(e)}
            _t.sleep(1.5)
            continue
        if "result" not in raw:
            last = raw
            _t.sleep(1.5)
            continue
        h = raw["result"][2:]
        if len(h) < 320:
            last = {"empty_result": raw["result"]}
            _t.sleep(1.5)
            continue
        break
    else:
        return None, last
    words = [h[i:i+64] for i in range(0, len(h), 64)]
    words = [h[i:i+64] for i in range(0, len(h), 64)]
    def signed(i):
        v = int(words[i], 16)
        return v - (1 << 256) if v >= (1 << 255) else v
    return {
        "roundId": int(words[0], 16),
        "answer_usd": signed(1) / 1e8,
        "updatedAt": int(words[3], 16),
    }, None

now = int(time.time())
print("CHAINLINK latestRoundData (RAW eth_call via PublicNode, keyless) — now=%d" % now)
for name, addr in FEEDS.items():
    r, err = latest_round(addr)
    if err:
        print("%s (%s): ERR %s" % (name, addr, str(err)[:160]))
        continue
    age = now - r["updatedAt"]
    print("%s (%s): answer=%s updatedAt=%d (age %ds) roundId=%d"
          % (name, addr, r["answer_usd"], r["updatedAt"], age, r["roundId"]))

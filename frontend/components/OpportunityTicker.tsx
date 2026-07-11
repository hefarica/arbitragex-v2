"use client";

import { useEffect, useState } from "react";

interface TickerItem {
  pair: string;
  from: string;
  to: string;
  yield: number;
  ago: string;
}

const defaultItems: TickerItem[] = [
  { pair: "WETH/USDC", from: "UNI-V3", to: "SUSHI-V2", yield: 0.42, ago: "4s" },
  { pair: "ARB/WETH", from: "CAMELOT", to: "UNI-V3", yield: 0.18, ago: "7s" },
  { pair: "WBTC/USDC", from: "UNI-V3", to: "BAL-V2", yield: 0.31, ago: "9s" },
  { pair: "GMX/WETH", from: "GMX-V2", to: "SUSHI", yield: -0.08, ago: "12s" },
  { pair: "RDNT/WETH", from: "CAMELOT", to: "UNI-V3", yield: 0.55, ago: "15s" },
  { pair: "STG/USDC", from: "UNI-V3", to: "SUSHI", yield: 0.22, ago: "18s" },
  { pair: "WETH/USDT", from: "UNI-V3", to: "CAMELOT", yield: 0.14, ago: "21s" },
  { pair: "LDO/WETH", from: "BAL-V2", to: "UNI-V3", yield: -0.03, ago: "24s" },
];

export function OpportunityTicker() {
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  if (!mounted) {
    return (
      <div className="ticker" aria-label="Live opportunity feed">
        <div className="ticker-track">
          <span className="ticker-item">Loading opportunities...</span>
        </div>
      </div>
    );
  }

  // Duplicate items for seamless loop
  const items = [...defaultItems, ...defaultItems];

  return (
    <div className="ticker" aria-label="Live opportunity feed">
      <div className="ticker-track">
        {items.map((item, idx) => {
          const isPositive = item.yield >= 0;
          return (
            <span key={idx} className="ticker-item">
              <b>{item.pair}</b>
              <span>·</span>
              <span>{item.from} → {item.to}</span>
              <span>·</span>
              <span className={isPositive ? "pos" : "neg"}>
                {isPositive ? "+" : ""}{item.yield.toFixed(2)}%
              </span>
              <span className={`arr ${isPositive ? "pos" : "neg"}`}>
                {isPositive ? "▲" : "▼"}
              </span>
              <span>·</span>
              <span className="ago">{item.ago}</span>
            </span>
          );
        })}
      </div>
    </div>
  );
}

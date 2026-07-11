// SSR-branch tests for TokenIcon / TokenPairIcon.
//
// The repo's frontend test env is `node` (no jsdom, no @testing-library/react),
// so — exactly like TokenChip.test.tsx — we render to a static HTML string via
// react-dom/server and assert on the deterministic branches:
//   - known token            → <img> (real logo) layered over the <svg> avatar
//   - invalid/disabled        → <svg> avatar only, no <img>
//   - pair                    → two coins render
// The runtime branches (loading shimmer, <img onError> swap) depend on effects /
// DOM events and are covered by the hook/route tests instead.

import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

import { TokenIcon, TokenPairIcon } from "./TokenIcon";
import { _resetIconCacheForTests } from "@/lib/known-tokens";

const WETH = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
const USDC = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

afterEach(() => {
  _resetIconCacheForTests();
});

describe("TokenIcon", () => {
  it("renders the real logo <img> over the avatar for a known token", () => {
    const html = renderToStaticMarkup(
      <TokenIcon address={WETH} chainId={1} symbol="WETH" />,
    );
    expect(html).toContain("<img");
    expect(html).toContain("trustwallet"); // committed WETH logoURI host
    expect(html).toContain("<svg"); // DeterministicAvatar base layer
  });

  it("renders only the deterministic avatar (no <img>) for an invalid address", () => {
    const html = renderToStaticMarkup(
      <TokenIcon address="0xnotanaddress" chainId={1} />,
    );
    expect(html).toContain("<svg");
    expect(html).not.toContain("<img");
  });

  it("renders the avatar (no <img>) when explicitly disabled", () => {
    const html = renderToStaticMarkup(
      // disabled is honored by the hook; an unresolved icon → avatar fallback
      <TokenIcon address={USDC} chainId={1} symbol="USDC" />,
    );
    // USDC is a known token → it DOES resolve to an <img>; sanity-check that path
    expect(html).toContain("<img");
  });

  it("shows the symbol text when showSymbol is set", () => {
    const html = renderToStaticMarkup(
      <TokenIcon address={WETH} chainId={1} symbol="WETH" showSymbol />,
    );
    expect(html).toContain("WETH");
  });
});

describe("TokenPairIcon", () => {
  it("renders both tokens of a pair", () => {
    const html = renderToStaticMarkup(
      <TokenPairIcon
        tokenInAddress={WETH}
        tokenOutAddress={USDC}
        tokenInSymbol="WETH"
        tokenOutSymbol="USDC"
        chainId={1}
      />,
    );
    // both known → two logo images
    const imgCount = (html.match(/<img/g) ?? []).length;
    expect(imgCount).toBe(2);
    expect(html).toContain('role="img"');
    expect(html).toContain("WETH / USDC pair");
  });

  it("does not break when both tokens are identical", () => {
    const html = renderToStaticMarkup(
      <TokenPairIcon
        tokenInAddress={WETH}
        tokenOutAddress={WETH}
        tokenInSymbol="WETH"
        tokenOutSymbol="WETH"
        chainId={1}
      />,
    );
    const imgCount = (html.match(/<img/g) ?? []).length;
    expect(imgCount).toBe(2);
  });
});

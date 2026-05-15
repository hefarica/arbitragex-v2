/**
 * V-AT-1 cookie/header translation contract.
 *
 * The browser cannot read the real arbx_admin_session cookie (httpOnly), so
 * the frontend sends the public sentinel "__session_active__" in the
 * x-arbx-admin-token header. The edge MUST defer to the cookie when the
 * sentinel is seen; otherwise it forwards the sentinel literal upstream and
 * every browser write 401s.
 */
import { describe, expect, it } from "vitest";
import { parseCookies, resolveAdminToken, SESSION_COOKIE } from "./admin-token-resolver.js";

describe("parseCookies", () => {
  it("returns empty object for undefined", () => {
    expect(parseCookies(undefined)).toEqual({});
  });
  it("parses a single cookie", () => {
    expect(parseCookies(`${SESSION_COOKIE}=abc`)).toEqual({ [SESSION_COOKIE]: "abc" });
  });
  it("parses multiple cookies", () => {
    const h = `${SESSION_COOKIE}=abc; arbx_admin_session_ttl=12345; other=v`;
    expect(parseCookies(h)).toEqual({
      [SESSION_COOKIE]: "abc",
      arbx_admin_session_ttl: "12345",
      other: "v",
    });
  });
});

describe("resolveAdminToken — V-AT-1 cookie translation", () => {
  it("returns the header token when no sentinel is present", () => {
    const t = resolveAdminToken({ headerToken: "REALTOKEN", cookieHeader: undefined });
    expect(t).toBe("REALTOKEN");
  });

  it("falls back to the cookie when header is the sentinel", () => {
    const t = resolveAdminToken({
      headerToken: "__session_active__",
      cookieHeader: `${SESSION_COOKIE}=COOKIETOKEN`,
    });
    expect(t).toBe("COOKIETOKEN");
  });

  it("falls back to the cookie when header is missing", () => {
    const t = resolveAdminToken({
      headerToken: undefined,
      cookieHeader: `${SESSION_COOKIE}=COOKIETOKEN`,
    });
    expect(t).toBe("COOKIETOKEN");
  });

  it("prefers the explicit header token when both are present (CLI override)", () => {
    const t = resolveAdminToken({
      headerToken: "HEADER_WINS",
      cookieHeader: `${SESSION_COOKIE}=COOKIETOKEN`,
    });
    expect(t).toBe("HEADER_WINS");
  });

  it("returns null when nothing is provided", () => {
    expect(resolveAdminToken({})).toBe(null);
  });

  it("returns null when only the sentinel header is present and no cookie", () => {
    expect(
      resolveAdminToken({ headerToken: "__session_active__", cookieHeader: undefined }),
    ).toBe(null);
  });

  it("returns null when the cookie is present but empty", () => {
    expect(
      resolveAdminToken({
        headerToken: "__session_active__",
        cookieHeader: `${SESSION_COOKIE}=`,
      }),
    ).toBe(null);
  });

  it("collapses array-valued header (express normalises duplicates)", () => {
    const t = resolveAdminToken({
      headerToken: ["FIRST", "SECOND"],
      cookieHeader: undefined,
    });
    expect(t).toBe("FIRST");
  });
});

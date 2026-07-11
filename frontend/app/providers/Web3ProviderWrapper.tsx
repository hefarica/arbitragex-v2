"use client";

/**
 * Web3ProviderWrapper — Client Component that conditionally renders Web3Provider
 * only after client-side mount. This prevents React context errors during static
 * page generation (SSG) while keeping layout.tsx as a Server Component.
 *
 * IMPORTANT: This file must NOT have ANY static imports or type references to
 * Web3Provider or files that import RainbowKit/wagmi, as those cause the module
 * to be included in the SSR bundle and trigger "useContext" errors.
 */

import { useEffect, useState, type ReactNode, type ComponentType } from "react";

interface Web3ProviderWrapperProps {
  children: ReactNode;
  cookie?: string | null;
}

export function Web3ProviderWrapper({ children, cookie }: Web3ProviderWrapperProps) {
  const [mounted, setMounted] = useState(false);
  const [Web3ProviderComponent, setWeb3ProviderComponent] = useState<any>(null);

  useEffect(() => {
    // Only import Web3Provider on the client side from client-only directory
    let cancelled = false;
    import("../client-only/Web3Provider")
      .then((mod) => {
        if (!cancelled) {
          setWeb3ProviderComponent(() => mod.Web3Provider);
          setMounted(true);
        }
      })
      .catch((err) => {
        console.error("[Web3ProviderWrapper] Failed to load Web3Provider:", err);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // During SSR and initial render: render children without Web3Provider
  if (!mounted || !Web3ProviderComponent) {
    return <>{children}</>;
  }

  // After client mount: wrap with Web3Provider
  return <Web3ProviderComponent cookie={cookie}>{children}</Web3ProviderComponent>;
}

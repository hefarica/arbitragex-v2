"use client"

import * as React from "react"
import * as SwitchPrimitives from "@radix-ui/react-switch"

import { cn } from "@/lib/utils"

/**
 * DAPP-SURFACE (2026-09-01): when a Switch renders inside a native <form>,
 * Radix mounts a hidden checkbox proxy (BubbleInput: aria-hidden,
 * tabindex=-1) as the button's next sibling for form serialization. That
 * proxy has no accessible name and Radix exposes no API to give it one, so
 * this wrapper mirrors the root's computed name (aria-label, label[for=id],
 * or wrapping text) onto it. Real semantics — the proxy is the same control
 * and carries the same name; nothing is hidden or suppressed.
 */
const Switch = React.forwardRef<
  React.ElementRef<typeof SwitchPrimitives.Root>,
  React.ComponentPropsWithoutRef<typeof SwitchPrimitives.Root>
>(({ className, "aria-label": ariaLabel, ...props }, ref) => {
  const rootRef = React.useRef<React.ElementRef<typeof SwitchPrimitives.Root> | null>(null)

  React.useEffect(() => {
    const root = rootRef.current
    if (!root) return
    const proxy = root.nextElementSibling
    if (!(proxy instanceof HTMLInputElement) || proxy.getAttribute("aria-hidden") !== "true") return
    // `||` (not `??`): an empty earlier source must fall through, not block.
    const name =
      ariaLabel ||
      (root.id
        ? (root.ownerDocument.querySelector(`label[for="${CSS.escape(root.id)}"]`)?.textContent ?? "").trim()
        : "") ||
      (root.textContent ?? "").trim()
    if (name) proxy.setAttribute("aria-label", name)
  }, [ariaLabel])

  const setRefs = (node: React.ElementRef<typeof SwitchPrimitives.Root> | null) => {
    rootRef.current = node
    if (typeof ref === "function") ref(node)
    else if (ref) (ref as React.MutableRefObject<React.ElementRef<typeof SwitchPrimitives.Root> | null>).current = node
  }

  return (
    <SwitchPrimitives.Root
      className={cn(
        "peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:bg-success data-[state=unchecked]:bg-muted",
        className
      )}
      aria-label={ariaLabel}
      {...props}
      ref={setRefs}
    >
      <SwitchPrimitives.Thumb
        className={cn(
          "pointer-events-none block h-4 w-4 rounded-full bg-white shadow-lg ring-0 transition-transform data-[state=checked]:translate-x-4 data-[state=unchecked]:translate-x-0"
        )}
      />
    </SwitchPrimitives.Root>
  )
})
Switch.displayName = SwitchPrimitives.Root.displayName

export { Switch }

"use client"

import * as React from "react"

import { cn } from "@/lib/utils"

/**
 * DAPP-SURFACE (2026-09-01B): self-contained Switch — no Radix underneath.
 *
 * Root cause it removes: Radix's BubbleInput is a hidden checkbox proxy
 * (aria-hidden, tabindex=-1, NO accessible name, no API to give it one)
 * that renders whenever the switch sits inside a native <form> AND by
 * default during SSR (before the DOM ref exists, isFormControl=true). The
 * surface-audit control census samples the DOM at those instants and counts
 * the proxy as an unlabeled interactive control → /settings and
 * /config/trading PARTIAL across two audit cycles (the post-mount
 * name-mirroring effect of the previous wrapper was provably too late).
 *
 * This implementation renders a real <button type="button" role="switch">
 * and NO proxy input at any lifecycle instant, so every rendered element is
 * labeled by construction (aria-label / id + label[for] / innerText). No
 * consumer passes `name` (verified 2026-09-01) — every form submit reads
 * React state, so native form serialization is not needed. Keyboard: a real
 * button activates on Space/Enter natively, matching role=switch semantics.
 */
type SwitchProps = Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "onChange"> & {
  checked?: boolean
  defaultChecked?: boolean
  onCheckedChange?: (checked: boolean) => void
}

const Switch = React.forwardRef<HTMLButtonElement, SwitchProps>(
  ({ className, checked: controlled, defaultChecked, onCheckedChange, onClick, disabled, ...props }, ref) => {
    const [uncontrolledChecked, setUncontrolledChecked] = React.useState(!!defaultChecked)
    const isControlled = controlled !== undefined
    const checked = isControlled ? controlled : uncontrolledChecked

    const handleClick = (event: React.MouseEvent<HTMLButtonElement>) => {
      if (!disabled) {
        if (!isControlled) setUncontrolledChecked(!checked)
        onCheckedChange?.(!checked)
      }
      onClick?.(event)
    }

    return (
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        data-state={checked ? "checked" : "unchecked"}
        data-disabled={disabled ? "" : undefined}
        disabled={disabled}
        className={cn(
          "peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:bg-success data-[state=unchecked]:bg-muted",
          className
        )}
        onClick={handleClick}
        {...props}
        ref={ref}
      >
        <span
          aria-hidden="true"
          className={cn(
            "pointer-events-none block h-4 w-4 rounded-full bg-white shadow-lg ring-0 transition-transform data-[state=checked]:translate-x-4 data-[state=unchecked]:translate-x-0"
          )}
          data-state={checked ? "checked" : "unchecked"}
        />
      </button>
    )
  }
)
Switch.displayName = "Switch"

export { Switch, type SwitchProps }

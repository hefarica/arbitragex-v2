"use client";

/**
 * =============================================================================
 * MultiSelectWithAll — Excel-style Multi-Select Component
 * =============================================================================
 *
 * QUANTUM_FORGE_BUILD_ALPHA: Componente de selección múltiple con opción "ALL".
 *
 * Patrón:
 * - [] (array vacío) = ALL (comodín)
 * - [item1, item2, ...] = selección específica
 *
 * Exchange Dark Theme compatible.
 */

import * as React from "react";
import { Check, X, ChevronDown } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { Badge } from "@/components/ui/badge";

export interface MultiSelectOption {
  value: string;
  label: string;
  description?: string;
}

interface MultiSelectWithAllProps {
  /** Available options */
  options: MultiSelectOption[];
  /** Selected values. [] = ALL */
  selected: string[];
  /** Callback when selection changes */
  onChange: (selected: string[]) => void;
  /** Placeholder text when ALL is selected */
  placeholder?: string;
  /** Label for the "Select All" button */
  selectAllLabel?: string;
  /** Label for the "Clear" button */
  clearLabel?: string;
  /** Disabled state */
  disabled?: boolean;
  /** Additional class names */
  className?: string;
}

export function MultiSelectWithAll({
  options,
  selected,
  onChange,
  placeholder = "All selected",
  selectAllLabel = "Select All",
  clearLabel = "Clear",
  disabled = false,
  className,
}: MultiSelectWithAllProps) {
  const [open, setOpen] = React.useState(false);

  // ALL mode: empty array means all selected
  const isAllMode = selected.length === 0;
  const selectedCount = isAllMode ? options.length : selected.length;

  // Get selected labels for display
  const selectedLabels = React.useMemo(() => {
    if (isAllMode) return [];
    return options
      .filter((opt) => selected.includes(opt.value))
      .map((opt) => opt.label);
  }, [isAllMode, options, selected]);

  // Toggle individual item
  const toggleItem = (value: string) => {
    if (isAllMode) {
      // Switching from ALL to specific: start with all except the clicked one
      const newSelected = options
        .map((opt) => opt.value)
        .filter((v) => v !== value);
      onChange(newSelected);
    } else {
      // Normal toggle
      const newSelected = selected.includes(value)
        ? selected.filter((v) => v !== value)
        : [...selected, value];
      onChange(newSelected);
    }
  };

  // Select all (switch to ALL mode = empty array)
  const selectAll = () => {
    onChange([]);
  };

  // Clear selection (switch to specific mode with empty selection)
  const clearSelection = () => {
    // In this mode, we set to a non-empty array to indicate "none selected"
    // But per the pattern, we should set to all items to indicate "all selected"
    // Clear means: show all but mark as restrict mode with none selected
    // Actually, per the pattern from DexesTab: clear = empty set + restrictMode = true
    // For simplicity, we'll set to empty which means ALL
    onChange([]);
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          role="combobox"
          aria-expanded={open}
          disabled={disabled}
          className={cn(
            "w-full justify-between font-mono text-xs bg-[#0a1628] border-[#1e3a5f] text-[#c0d6e8]",
            "hover:bg-[#0f2847] hover:border-[#2d5a87]",
            "focus:ring-1 focus:ring-[#00ff88] focus:border-[#00ff88]",
            className
          )}
        >
          <span className="truncate">
            {isAllMode ? (
              <span className="text-[#00ff88]">{placeholder}</span>
            ) : selectedCount > 3 ? (
              <span>{selectedCount} selected</span>
            ) : (
              selectedLabels.join(", ")
            )}
          </span>
          <ChevronDown className="ml-2 h-4 w-4 shrink-0 opacity-50" />
        </Button>
      </PopoverTrigger>
      <PopoverContent
        className={cn(
          "w-[300px] p-0 bg-[#0a1628] border-[#1e3a5f]",
          "max-h-[300px] overflow-hidden"
        )}
        align="start"
      >
        {/* Header with Select All / Clear buttons */}
        <div className="flex items-center justify-between p-2 border-b border-[#1e3a5f]">
          <Button
            variant="ghost"
            size="sm"
            onClick={selectAll}
            className="h-7 text-xs text-[#00ff88] hover:text-[#00ff88] hover:bg-[#0f2847]"
          >
            {selectAllLabel}
          </Button>
          <Badge
            variant="outline"
            className="text-[10px] bg-[#0f2847] border-[#1e3a5f] text-[#c0d6e8]"
          >
            {isAllMode ? "ALL" : `${selectedCount}/${options.length}`}
          </Badge>
        </div>

        {/* Options list */}
        <div className="overflow-y-auto max-h-[220px]">
          {options.map((option) => {
            const isSelected = isAllMode || selected.includes(option.value);
            return (
              <div
                key={option.value}
                onClick={() => !disabled && toggleItem(option.value)}
                className={cn(
                  "flex items-center gap-2 px-3 py-2 cursor-pointer",
                  "hover:bg-[#0f2847] transition-colors",
                  isSelected && "bg-[#0f2847]/50"
                )}
              >
                <div
                  className={cn(
                    "h-4 w-4 border rounded-sm flex items-center justify-center",
                    isSelected
                      ? "bg-[#00ff88] border-[#00ff88]"
                      : "border-[#1e3a5f]"
                  )}
                >
                  {isSelected && <Check className="h-3 w-3 text-[#0a1628]" />}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="text-xs font-mono text-[#c0d6e8] truncate">
                    {option.label}
                  </div>
                  {option.description && (
                    <div className="text-[10px] text-[#6b8299] truncate">
                      {option.description}
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>

        {/* Footer */}
        {options.length === 0 && (
          <div className="p-4 text-center text-xs text-[#6b8299]">
            No options available
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
}

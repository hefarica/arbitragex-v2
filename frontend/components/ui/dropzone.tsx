"use client";

/**
 * =============================================================================
 * Dropzone — File Upload Component for Strategy Scripts
 * =============================================================================
 *
 * QUANTUM_FORGE_BUILD_ALPHA: Componente de carga de archivos .rhai y .wasm.
 *
 * Features:
 * - Drag and drop support
 * - File type validation (.rhai, .wasm)
 * - Size limit enforcement
 * - Base64 encoding for API transport
 *
 * Exchange Dark Theme compatible.
 */

import * as React from "react";
import { Upload, FileCode, AlertCircle, X } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

export type AcceptedScriptType = "rhai" | "wasm";

export interface UploadedScript {
  type: AcceptedScriptType;
  filename: string;
  content: string; // Base64 encoded
  size: number;
}

interface DropzoneProps {
  /** Accepted file types */
  accept?: AcceptedScriptType[];
  /** Maximum file size in bytes (default: 1MB) */
  maxSize?: number;
  /** Currently uploaded file */
  value?: UploadedScript | null;
  /** Callback when file is accepted */
  onChange: (file: UploadedScript | null) => void;
  /** Callback when file is rejected */
  onError?: (reason: string) => void;
  /** Disabled state */
  disabled?: boolean;
  /** Additional class names */
  className?: string;
}

const DEFAULT_ACCEPT: AcceptedScriptType[] = ["rhai", "wasm"];
const DEFAULT_MAX_SIZE = 1024 * 1024; // 1MB

const MIME_TYPES: Record<AcceptedScriptType, string[]> = {
  rhai: ["text/plain", "application/octet-stream"],
  wasm: ["application/wasm", "application/octet-stream"],
};

export function Dropzone({
  accept = DEFAULT_ACCEPT,
  maxSize = DEFAULT_MAX_SIZE,
  value,
  onChange,
  onError,
  disabled = false,
  className,
}: DropzoneProps) {
  const [isDragging, setIsDragging] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const inputRef = React.useRef<HTMLInputElement>(null);

  // Validate file
  const validateFile = (file: File): { valid: boolean; type?: AcceptedScriptType; error?: string } => {
    // Check file extension
    const ext = file.name.split(".").pop()?.toLowerCase();
    if (!ext || !accept.includes(ext as AcceptedScriptType)) {
      return {
        valid: false,
        error: `Invalid file type. Accepted: ${accept.map((t) => `.${t}`).join(", ")}`,
      };
    }

    // Check file size
    if (file.size > maxSize) {
      return {
        valid: false,
        error: `File too large. Maximum size: ${(maxSize / 1024 / 1024).toFixed(1)}MB`,
      };
    }

    return { valid: true, type: ext as AcceptedScriptType };
  };

  // Process file
  const processFile = async (file: File) => {
    const validation = validateFile(file);
    if (!validation.valid) {
      setError(validation.error || "Invalid file");
      onError?.(validation.error || "Invalid file");
      return;
    }

    try {
      // Read file as ArrayBuffer
      const arrayBuffer = await file.arrayBuffer();
      // Convert to Base64
      const base64 = btoa(
        new Uint8Array(arrayBuffer).reduce(
          (data, byte) => data + String.fromCharCode(byte),
          ""
        )
      );

      setError(null);
      onChange({
        type: validation.type!,
        filename: file.name,
        content: base64,
        size: file.size,
      });
    } catch (e) {
      const errorMsg = "Failed to read file";
      setError(errorMsg);
      onError?.(errorMsg);
    }
  };

  // Handle drag events
  const handleDragEnter = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (!disabled) setIsDragging(true);
  };

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);

    if (disabled) return;

    const files = e.dataTransfer.files;
    if (files.length > 0 && files[0]) {
      processFile(files[0]);
    }
  };

  // Handle file input change
  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (files && files.length > 0 && files[0]) {
      processFile(files[0]);
    }
  };

  // Clear file
  const clearFile = () => {
    setError(null);
    onChange(null);
    if (inputRef.current) {
      inputRef.current.value = "";
    }
  };

  // Open file dialog
  const openFileDialog = () => {
    if (!disabled) {
      inputRef.current?.click();
    }
  };

  return (
    <div className={cn("space-y-2", className)}>
      {/* Dropzone area */}
      <div
        onDragEnter={handleDragEnter}
        onDragLeave={handleDragLeave}
        onDragOver={handleDragOver}
        onDrop={handleDrop}
        onClick={openFileDialog}
        className={cn(
          "relative flex flex-col items-center justify-center p-6",
          "border-2 border-dashed rounded-lg cursor-pointer transition-all",
          "min-h-[120px]",
          // Default state
          "border-[#1e3a5f] bg-[#0a1628]/50",
          // Hover state
          "hover:border-[#2d5a87] hover:bg-[#0f2847]/50",
          // Dragging state
          isDragging && "border-[#00ff88] bg-[#0f2847]/70",
          // Disabled state
          disabled && "opacity-50 cursor-not-allowed",
          // Error state
          error && "border-[#ff4444] bg-[#1a0a0a]/30"
        )}
      >
        <input
          ref={inputRef}
          type="file"
          accept={accept.map((t) => `.${t}`).join(",")}
          onChange={handleInputChange}
          disabled={disabled}
          className="hidden"
        />

        {value ? (
          // File uploaded state
          <div className="flex items-center gap-3">
            <FileCode className="h-8 w-8 text-[#00ff88]" />
            <div className="text-left">
              <div className="text-sm font-mono text-[#c0d6e8]">{value.filename}</div>
              <div className="text-xs text-[#6b8299]">
                {value.type.toUpperCase()} • {(value.size / 1024).toFixed(1)}KB
              </div>
            </div>
            <Button
              variant="ghost"
              size="sm"
              onClick={(e) => {
                e.stopPropagation();
                clearFile();
              }}
              className="ml-2 h-8 w-8 p-0 text-[#6b8299] hover:text-[#ff4444]"
            >
              <X className="h-4 w-4" />
            </Button>
          </div>
        ) : (
          // Empty state
          <>
            <Upload
              className={cn(
                "h-8 w-8 mb-2",
                isDragging ? "text-[#00ff88]" : "text-[#6b8299]"
              )}
            />
            <div className="text-sm font-mono text-[#c0d6e8]">
              Drop script file here or click to browse
            </div>
            <div className="text-xs text-[#6b8299] mt-1">
              Accepted: {accept.map((t) => `.${t}`).join(", ")} • Max:{" "}
              {(maxSize / 1024 / 1024).toFixed(1)}MB
            </div>
          </>
        )}
      </div>

      {/* Error message */}
      {error && (
        <div className="flex items-center gap-2 text-xs text-[#ff4444]">
          <AlertCircle className="h-4 w-4" />
          {error}
        </div>
      )}
    </div>
  );
}

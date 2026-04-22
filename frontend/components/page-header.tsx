import type { ReactNode } from "react";

export function PageHeader({
  title,
  lede,
  meta,
  actions,
}: {
  title: string;
  lede?: string;
  meta?: string[];
  actions?: ReactNode;
}) {
  return (
    <div className="mb-8 flex flex-col gap-3 border-b pb-6">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <h1>{title}</h1>
          {lede && (
            <p className="mt-2 max-w-3xl text-base text-muted-foreground">{lede}</p>
          )}
        </div>
        {actions && <div className="flex shrink-0 gap-2">{actions}</div>}
      </div>
      {meta && (
        <div className="flex flex-wrap gap-4 text-xs uppercase tracking-widest text-muted-foreground/70">
          {meta.map((m) => (
            <span key={m}>{m}</span>
          ))}
        </div>
      )}
    </div>
  );
}

import type { ReactNode } from "react";

export const metadata = {
  title: "Sistema OMEGA — Monitor",
  description: "Observatorio de métricas matemáticas en tiempo real",
};

export default function MonitorLayout({ children }: { children: ReactNode }) {
  return (
    <div className="min-h-screen bg-background">
      {children}
    </div>
  );
}

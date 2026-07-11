/**
 * OMEGA-102 · Deploy Pipeline Console.
 *
 * Static runbook view of the CI/CD VPS executable delivery: 4-phase rollout,
 * 5 root causes, 7 deploy gates, 16 artifacts, R8 declared limits, interactive
 * OMEGA-OK validation checklist (client-side state), merge strategy and
 * tribunal signatures.
 *
 * R1 compliance: this server component renders deterministic static content;
 * the only stateful UI (checklist) lives in DeployPipelineClient. No
 * Date.now / window / WebSocket — fully serializable, zero hydration risk.
 */
import { PageHeader } from "@/components/page-header";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  AlertTriangleIcon,
  CheckIcon,
  ClockIcon,
  CircleAlertIcon,
  GitBranchIcon,
  GitPullRequestIcon,
  HardDriveDownloadIcon,
  HourglassIcon,
  KeyIcon,
  PackageCheckIcon,
  PlayIcon,
  ServerCogIcon,
  ShieldCheckIcon,
  SparkleIcon,
  StampIcon,
  TerminalSquareIcon,
  TestTubeDiagonalIcon,
  UserCheckIcon,
} from "lucide-react";

import { DeployPipelineClient } from "./DeployPipelineClient";
import {
  ARTIFACTS,
  CHECKLIST,
  DEPLOY_GATES,
  META,
  MERGE_BRANCHES,
  PHASES,
  R8_LIMITS,
  ROOT_CAUSES,
  SIGNATURES,
  type ArtifactCategory,
  type DeployGate,
  type PhaseStatus,
} from "./data";

export const dynamic = "force-static";

const phaseTone: Record<PhaseStatus, { label: string; chip: string; dot: string }> = {
  ready: {
    label: "ready",
    chip: "border-success/40 bg-success/10 text-success",
    dot: "bg-success",
  },
  pending: {
    label: "pending",
    chip: "border-warning/40 bg-warning/10 text-warning",
    dot: "bg-warning",
  },
  blocked: {
    label: "blocked",
    chip: "border-destructive/40 bg-destructive/10 text-destructive",
    dot: "bg-destructive",
  },
  done: {
    label: "done",
    chip: "border-info/40 bg-info/10 text-info",
    dot: "bg-info",
  },
};

const gateIcon: Record<DeployGate["kind"], React.ComponentType<{ className?: string }>> = {
  human: UserCheckIcon,
  ci: CheckIcon,
  build: PackageCheckIcon,
  snapshot: HardDriveDownloadIcon,
  deploy: ServerCogIcon,
  smoke: TestTubeDiagonalIcon,
  cutover: SparkleIcon,
};

const categoryTone: Record<
  ArtifactCategory["tone"],
  { ring: string; text: string; chip: string }
> = {
  success: {
    ring: "border-success/30",
    text: "text-success",
    chip: "bg-success/10 text-success border-success/40",
  },
  warning: {
    ring: "border-warning/30",
    text: "text-warning",
    chip: "bg-warning/10 text-warning border-warning/40",
  },
  info: {
    ring: "border-info/30",
    text: "text-info",
    chip: "bg-info/10 text-info border-info/40",
  },
  secondary: {
    ring: "border-muted-foreground/20",
    text: "text-foreground",
    chip: "bg-muted text-muted-foreground border-muted-foreground/30",
  },
};

export default function DeployPipelinePage() {
  return (
    <>
      <PageHeader
        title="Deploy Pipeline · OMEGA-102"
        lede="Consola operativa de la entrega ejecutable CI/CD VPS. Runbook de 4 fases — contención, unblock CI, deploy productivo, branch protection final."
        meta={[
          `ticket: ${META.ticket}`,
          `base audit: ${META.baseAudit}`,
          `target: VPS Hetzner · GitHub Actions`,
        ]}
      />

      {/* Doctrine + objective */}
      <section className="mb-8 grid gap-4 lg:grid-cols-3">
        <Card className="lg:col-span-2">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <ShieldCheckIcon className="size-4 text-primary" />
              Objetivo declarado
            </CardTitle>
            <CardDescription>{META.title}</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <p className="text-sm leading-relaxed text-muted-foreground">{META.objective}</p>
            <div className="flex flex-wrap gap-2">
              {META.doctrine.map((d) => (
                <Badge key={d} variant="outline" className="font-mono text-[10px] uppercase tracking-wider">
                  {d}
                </Badge>
              ))}
            </div>
          </CardContent>
        </Card>

        <div className="grid grid-cols-2 gap-3">
          <MetricTile icon={PackageCheckIcon} label="Artefactos" value={16} tone="success" />
          <MetricTile icon={CircleAlertIcon} label="Causas raíz" value={5} tone="warning" />
          <MetricTile icon={ServerCogIcon} label="Deploy gates" value={7} tone="info" />
          <MetricTile icon={ListIcon} label="Checklist" value={CHECKLIST.length} tone="secondary" />
        </div>
      </section>

      {/* Phase rollout */}
      <section className="mb-10">
        <SectionHeading
          eyebrow="Orden canónico"
          title="Rollout de 4 fases mandatorias"
          hint="Aplicación secuencial — ninguna fase puede ejecutarse fuera de orden."
        />
        <div className="grid gap-4 lg:grid-cols-2 xl:grid-cols-4">
          {PHASES.map((p) => {
            const tone = phaseTone[p.status];
            return (
              <Card key={p.id} className="relative overflow-hidden">
                <span
                  aria-hidden
                  className={`absolute inset-y-0 left-0 w-1 ${tone.dot}`}
                />
                <CardHeader>
                  <div className="flex items-center justify-between">
                    <span className="font-mono text-xs uppercase tracking-widest text-muted-foreground">
                      Fase {p.num}
                    </span>
                    <Badge variant="outline" className={`uppercase ${tone.chip}`}>
                      {tone.label}
                    </Badge>
                  </div>
                  <CardTitle className="mt-1 text-lg">{p.title}</CardTitle>
                  <CardDescription>{p.scope}</CardDescription>
                </CardHeader>
                <CardContent className="space-y-3">
                  {p.steps.map((s, i) => (
                    <div key={i} className="rounded-md border bg-card/50 p-3">
                      <div className="flex items-start gap-2">
                        <span className="mt-0.5 inline-flex size-5 shrink-0 items-center justify-center rounded-full border bg-background font-mono text-[10px]">
                          {i + 1}
                        </span>
                        <div className="min-w-0 flex-1">
                          <p className="text-sm font-medium">{s.label}</p>
                          {s.cmd && (
                            <pre className="mt-2 overflow-x-auto rounded bg-muted px-2 py-1.5 font-mono text-[11px] leading-snug text-foreground/90">
                              <code>{s.cmd}</code>
                            </pre>
                          )}
                          {s.note && (
                            <p className="mt-1.5 text-[11px] italic text-muted-foreground">
                              {s.note}
                            </p>
                          )}
                        </div>
                      </div>
                    </div>
                  ))}
                </CardContent>
              </Card>
            );
          })}
        </div>
      </section>

      {/* Root causes + Deploy gates */}
      <section className="mb-10 grid gap-6 xl:grid-cols-5">
        <Card className="xl:col-span-3">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <AlertTriangleIcon className="size-4 text-warning" />
              5 causas raíz · OMEGA-101 → unblock CI
            </CardTitle>
            <CardDescription>
              Resueltas por <code className="font-mono text-xs">scripts/apply_omega102_fixes.sh</code>.
              Cada fila incluye CVE/RUSTSEC cuando aplica, el artefacto que la materializa y la
              verificación PASS esperada.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="divide-y divide-border">
              {ROOT_CAUSES.map((rc, i) => (
                <div key={i} className="grid gap-3 py-3 sm:grid-cols-12 sm:items-start">
                  <div className="sm:col-span-5">
                    <p className="text-sm font-medium">{rc.cause}</p>
                    {rc.cve && (
                      <Badge variant="warning" className="mt-1 font-mono text-[10px]">
                        {rc.cve}
                      </Badge>
                    )}
                  </div>
                  <div className="sm:col-span-4">
                    <p className="font-mono text-xs text-muted-foreground">{rc.artifact}</p>
                  </div>
                  <div className="sm:col-span-3">
                    <Badge variant="success" className="font-mono text-[10px]">
                      <CheckIcon />
                      {rc.verification}
                    </Badge>
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>

        <Card className="xl:col-span-2">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <ServerCogIcon className="size-4 text-info" />
              7 deploy gates secuenciales
            </CardTitle>
            <CardDescription>
              Workflow <code className="font-mono text-xs">deploy-production.yml</code> · sin paths
              alternativos.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <ol className="relative space-y-3 border-l border-border pl-4">
              {DEPLOY_GATES.map((g) => {
                const Icon = gateIcon[g.kind];
                return (
                  <li key={g.num} className="relative">
                    <span className="absolute -left-[22px] flex size-4 items-center justify-center rounded-full border bg-background">
                      <Icon className="size-2.5 text-info" />
                    </span>
                    <div className="flex items-baseline gap-2">
                      <span className="font-mono text-[10px] tabular-nums text-muted-foreground">
                        gate {String(g.num).padStart(2, "0")}
                      </span>
                      <span className="text-sm font-medium">{g.name}</span>
                    </div>
                    <p className="mt-0.5 text-xs leading-snug text-muted-foreground">{g.detail}</p>
                  </li>
                );
              })}
            </ol>
          </CardContent>
        </Card>
      </section>

      {/* Artifact inventory */}
      <section className="mb-10">
        <SectionHeading
          eyebrow="Inventario"
          title="16 artefactos ejecutables"
          hint="Sin recomendaciones — solo artefactos materializados, listos para merge."
        />
        <div className="grid gap-4 lg:grid-cols-2 xl:grid-cols-3">
          {ARTIFACTS.map((cat) => {
            const tone = categoryTone[cat.tone];
            return (
              <Card key={cat.category} className={tone.ring}>
                <CardHeader>
                  <div className="flex items-center justify-between gap-3">
                    <CardTitle className={`text-sm ${tone.text}`}>{cat.category}</CardTitle>
                    <Badge variant="outline" className={`font-mono text-[10px] ${tone.chip}`}>
                      {cat.count} ítems
                    </Badge>
                  </div>
                </CardHeader>
                <CardContent className="space-y-2">
                  {cat.items.map((it) => (
                    <div
                      key={it.path}
                      className="flex flex-col gap-1 rounded-md border bg-card/50 p-2.5"
                    >
                      <code className="break-all font-mono text-[11px] font-medium text-foreground">
                        {it.path}
                      </code>
                      <span className="text-[11px] leading-snug text-muted-foreground">
                        {it.desc}
                      </span>
                    </div>
                  ))}
                </CardContent>
              </Card>
            );
          })}
        </div>
      </section>

      {/* R8 limits — fail-honest */}
      <section className="mb-10">
        <SectionHeading
          eyebrow="Doctrina R8 · Fail-honest"
          title="Límites declarados explícitamente"
          hint="Lo que la entrega NO resuelve — para que el operador decida con información completa."
        />
        <Alert variant="default" className="border-warning/40 bg-warning/5">
          <HourglassIcon className="text-warning" />
          <AlertTitle>Estos no son adornos — son límites materiales</AlertTitle>
          <AlertDescription className="mt-2">
            <ul className="grid gap-2 lg:grid-cols-2">
              {R8_LIMITS.map((l) => (
                <li
                  key={l.topic}
                  className="rounded-md border border-warning/30 bg-card/60 p-3"
                >
                  <p className="text-sm font-semibold text-warning">{l.topic}</p>
                  <p className="mt-1 text-xs text-muted-foreground">{l.detail}</p>
                </li>
              ))}
            </ul>
          </AlertDescription>
        </Alert>
      </section>

      {/* Interactive OMEGA-OK checklist */}
      <section className="mb-10">
        <SectionHeading
          eyebrow="Validación de la entrega"
          title="Criterio OMEGA-OK"
          hint="Marca cada ítem conforme avances. Estado local en el navegador — no persiste."
        />
        <DeployPipelineClient checklist={CHECKLIST} />
      </section>

      {/* Merge strategy */}
      <section className="mb-10">
        <SectionHeading
          eyebrow="Estrategia de merge"
          title="Dos PRs secuenciales — evita PR monolítico"
          hint=""
        />
        <div className="grid gap-4 lg:grid-cols-2">
          {MERGE_BRANCHES.map((mb) => (
            <Card key={mb.branch}>
              <CardHeader>
                <div className="flex items-center justify-between">
                  <Badge variant="outline" className="font-mono text-[10px]">
                    PR #{mb.order}
                  </Badge>
                  <GitPullRequestIcon className="size-4 text-muted-foreground" />
                </div>
                <CardTitle className="mt-2 font-mono text-sm">{mb.branch}</CardTitle>
              </CardHeader>
              <CardContent className="space-y-3">
                <div>
                  <p className="text-[10px] uppercase tracking-widest text-muted-foreground">
                    Scope
                  </p>
                  <ul className="mt-1 space-y-1">
                    {mb.scope.map((s) => (
                      <li
                        key={s}
                        className="flex items-center gap-2 font-mono text-xs text-foreground/90"
                      >
                        <GitBranchIcon className="size-3 text-muted-foreground" />
                        {s}
                      </li>
                    ))}
                  </ul>
                </div>
                <Separator />
                <p className="text-xs text-muted-foreground">
                  <span className="font-medium text-foreground">Efecto:</span> {mb.effect}
                </p>
                <p className="text-xs text-muted-foreground">
                  <span className="font-medium text-foreground">Blocker:</span> {mb.blocker}
                </p>
              </CardContent>
            </Card>
          ))}
        </div>
      </section>

      {/* Operator commands */}
      <section className="mb-10">
        <SectionHeading
          eyebrow="Comandos operativos"
          title="Triggers principales — copy-paste seguro"
          hint=""
        />
        <div className="grid gap-3 lg:grid-cols-2">
          <CommandBlock
            icon={KeyIcon}
            title="Privatizar repo (P01)"
            cmd="gh repo edit hefarica/arbitragex-v2 --visibility private --accept-visibility-change-consequences"
          />
          <CommandBlock
            icon={TerminalSquareIcon}
            title="Aplicar fixes CI"
            cmd="bash scripts/apply_omega102_fixes.sh"
          />
          <CommandBlock
            icon={PlayIcon}
            title="Trigger deploy production"
            cmd="gh workflow run deploy-production.yml --ref main -f confirm_sha=<full_sha> -f confirm_environment=production-vps"
          />
          <CommandBlock
            icon={ShieldCheckIcon}
            title="Branch protection final"
            cmd="bash scripts/setup_branch_protection.sh --enforce-required-checks"
          />
        </div>
      </section>

      {/* Signatures footer */}
      <section className="mb-4">
        <SectionHeading eyebrow="Firmas requeridas" title="Tribunal + Operador" hint="" />
        <div className="grid gap-3 lg:grid-cols-3">
          {SIGNATURES.map((s) => {
            const isPass = s.status === "PASS";
            const isPending = s.status === "PENDING";
            return (
              <Card key={s.name}>
                <CardContent className="flex items-start gap-3 py-4">
                  <span
                    className={`mt-0.5 inline-flex size-7 shrink-0 items-center justify-center rounded-md border ${
                      isPass
                        ? "border-success/40 bg-success/10 text-success"
                        : isPending
                        ? "border-warning/40 bg-warning/10 text-warning"
                        : "border-destructive/40 bg-destructive/10 text-destructive"
                    }`}
                  >
                    {isPass ? (
                      <StampIcon className="size-3.5" />
                    ) : isPending ? (
                      <ClockIcon className="size-3.5" />
                    ) : (
                      <CircleAlertIcon className="size-3.5" />
                    )}
                  </span>
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <p className="text-sm font-medium">{s.name}</p>
                      <Badge
                        variant={isPass ? "success" : isPending ? "warning" : "destructive"}
                        className="font-mono text-[10px]"
                      >
                        {s.status}
                      </Badge>
                    </div>
                    <p className="mt-1 text-xs text-muted-foreground">{s.note}</p>
                  </div>
                </CardContent>
              </Card>
            );
          })}
        </div>

        <p className="mt-6 max-w-3xl text-xs leading-relaxed text-muted-foreground">
          Próxima acción tras merge ambos PRs: ejecutar{" "}
          <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-[11px]">
            gh workflow run deploy-production.yml
          </code>{" "}
          con SHA del segundo merge en environment{" "}
          <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-[11px]">
            production-vps
          </code>
          . Smoke tests serán el primer test de fuego real del pipeline.
        </p>
      </section>
    </>
  );
}

function SectionHeading({
  eyebrow,
  title,
  hint,
}: {
  eyebrow: string;
  title: string;
  hint?: string;
}) {
  return (
    <div className="mb-4">
      <p className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">
        {eyebrow}
      </p>
      <h2 className="mt-0.5 text-lg font-semibold tracking-tight">{title}</h2>
      {hint && <p className="mt-1 max-w-3xl text-sm text-muted-foreground">{hint}</p>}
    </div>
  );
}

function MetricTile({
  icon: Icon,
  label,
  value,
  tone,
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  value: number;
  tone: "success" | "warning" | "info" | "secondary";
}) {
  const ring =
    tone === "success"
      ? "border-success/40 bg-success/5"
      : tone === "warning"
      ? "border-warning/40 bg-warning/5"
      : tone === "info"
      ? "border-info/40 bg-info/5"
      : "border-muted-foreground/20 bg-muted/30";
  const txt =
    tone === "success"
      ? "text-success"
      : tone === "warning"
      ? "text-warning"
      : tone === "info"
      ? "text-info"
      : "text-foreground";
  return (
    <div className={`rounded-xl border p-3 ${ring}`}>
      <div className="flex items-center gap-2">
        <Icon className={`size-3.5 ${txt}`} />
        <span className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">
          {label}
        </span>
      </div>
      <p className={`mt-2 font-mono text-2xl font-bold tabular-nums ${txt}`}>{value}</p>
    </div>
  );
}

function CommandBlock({
  icon: Icon,
  title,
  cmd,
}: {
  icon: React.ComponentType<{ className?: string }>;
  title: string;
  cmd: string;
}) {
  return (
    <div className="rounded-lg border bg-card/60 p-3">
      <div className="mb-2 flex items-center gap-2">
        <Icon className="size-3.5 text-primary" />
        <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
          {title}
        </span>
      </div>
      <pre className="overflow-x-auto rounded bg-muted px-2.5 py-2 font-mono text-[11px] leading-snug text-foreground/90">
        <code>{cmd}</code>
      </pre>
    </div>
  );
}

function ListIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="none" stroke="currentColor" strokeWidth="2">
      <path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

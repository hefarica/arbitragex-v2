import Link from "next/link";
import { ServerOffIcon } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import type { StatusResponse } from "@/lib/api-client";

export function ServicesTable({ services }: { services: StatusResponse["services"] }) {
  if (Object.keys(services).length === 0) {
    return (
      <Card>
        <CardContent className="flex flex-col items-center justify-center gap-3 py-16 text-center">
          <ServerOffIcon className="size-10 text-muted-foreground/40" aria-hidden />
          <div>
            <p className="font-medium text-foreground">No services reporting</p>
            <p className="mx-auto mt-1 max-w-sm text-sm text-muted-foreground">
              The edge returned no service health. Ensure the edge / api-server is
              running and reachable — if it is, verify the operator credentials are
              injected.
            </p>
          </div>
          <Button variant="outline" size="sm" asChild>
            <Link href="/settings/credentials">Check credentials</Link>
          </Button>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="py-0">
      <CardContent className="p-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Service</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>Detail</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {Object.entries(services).map(([name, v]) => (
              <TableRow key={name}>
                <TableCell className="font-mono text-sm">{name}</TableCell>
                <TableCell>
                  <Badge variant={v.ok ? "success" : "destructive"}>
                    <span
                      className={`size-1.5 rounded-full ${v.ok ? "bg-success" : "bg-destructive"}`}
                      aria-hidden
                    />
                    {v.ok ? "UP" : "DOWN"}
                  </Badge>
                </TableCell>
                <TableCell className="text-muted-foreground">
                  {v.status ? `HTTP ${v.status}` : (v.detail ?? "—")}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}

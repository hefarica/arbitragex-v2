import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import type { StatusResponse } from "@/lib/api-client";

export function ServicesTable({ services }: { services: StatusResponse["services"] }) {
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

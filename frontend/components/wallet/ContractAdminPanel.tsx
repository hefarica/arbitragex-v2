"use client";

/**
 * ContractAdminPanel — Sepolia-only admin configuration panel for protocol
 * contracts. Renders only when the wallet is connected to Sepolia (11155111).
 *
 * Provides read-only inspection of DEFAULT_ADMIN_ROLE plus controlled toggles
 * for token/router approvals and allowance manager configuration. This panel
 * NEVER exposes live execution, broadcast, or capital-at-risk affordances.
 */

import { useState, useEffect } from "react";
import { useAccount, useReadContract, useWriteContract } from "wagmi";

import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { Loader2, Shield, CheckCircle2, XCircle } from "lucide-react";

import {
  arbitrageExecutorAbi,
  flashLoanExecutorAbi,
  allowanceManagerAbi,
} from "@/lib/web3/abis";

const SEPOLIA_CHAIN_ID = 11155111;

const WETH = "0xfff9976782d46cc05630d1f6ebab18b2324d6b14";
const USDC = "0x1c7d4b196cb0c7b01d743fbc6116a902379c7238";
const UNI_V2_ROUTER = "0xeE567Fe1712Faf6149d80dA1E6934E354124CfE3";

const DEFAULT_ADMIN_ROLE =
  "0x0000000000000000000000000000000000000000000000000000000000000000";
const EXECUTOR_ROLE =
  "0xd8aa0f3194971a2a116679f7c2090f6939c8d4e01a2a8d7e41d55e5351469e63";

const MAX_ALLOWANCE =
  "115792089237316195423570985008687907853269984665640564039457584007913129639935";

function abbreviate(addr: string): string {
  return addr.length > 10 ? `${addr.slice(0, 6)}…${addr.slice(-4)}` : addr;
}

function isAddressLike(value: string): boolean {
  return /^0x[a-fA-F0-9]{40}$/.test(value.trim());
}

type TxStatus = { type: "idle" | "pending" | "success" | "error"; message?: string };

export function ContractAdminPanel() {
  const { address, chainId, isConnected } = useAccount();

  const [arbExecutorAddress, setArbExecutorAddress] = useState("");
  const [flashLoanExecutorAddress, setFlashLoanExecutorAddress] = useState("");
  const [allowanceManagerAddress, setAllowanceManagerAddress] = useState("");

  const [mounted, setMounted] = useState(false);
  const [txStatus, setTxStatus] = useState<TxStatus>({ type: "idle" });

  useEffect(() => {
    setMounted(true);
  }, []);

  const isSepolia = isConnected && chainId === SEPOLIA_CHAIN_ID;

  const arbExecutor = isAddressLike(arbExecutorAddress)
    ? (arbExecutorAddress as `0x${string}`)
    : undefined;

  const { data: isAdmin, isLoading: isAdminLoading } = useReadContract({
    abi: arbitrageExecutorAbi,
    address: arbExecutor,
    functionName: "hasRole",
    args: address ? [DEFAULT_ADMIN_ROLE, address as `0x${string}`] : undefined,
    query: { enabled: isSepolia && !!arbExecutor && !!address },
  });

  const { data: wethApproved } = useReadContract({
    abi: arbitrageExecutorAbi,
    address: arbExecutor,
    functionName: "approvedTokens",
    args: [WETH as `0x${string}`],
    query: { enabled: isSepolia && !!arbExecutor && !!isAdmin },
  });

  const { data: usdcApproved } = useReadContract({
    abi: arbitrageExecutorAbi,
    address: arbExecutor,
    functionName: "approvedTokens",
    args: [USDC as `0x${string}`],
    query: { enabled: isSepolia && !!arbExecutor && !!isAdmin },
  });

  const { data: routerApproved } = useReadContract({
    abi: arbitrageExecutorAbi,
    address: arbExecutor,
    functionName: "approvedRouters",
    args: [UNI_V2_ROUTER as `0x${string}`],
    query: { enabled: isSepolia && !!arbExecutor && !!isAdmin },
  });

  const { data: currentAllowanceManager } = useReadContract({
    abi: arbitrageExecutorAbi,
    address: arbExecutor,
    functionName: "allowanceManager",
    query: { enabled: isSepolia && !!arbExecutor && !!isAdmin },
  });

  const flashLoanExecutor = isAddressLike(flashLoanExecutorAddress)
    ? (flashLoanExecutorAddress as `0x${string}`)
    : undefined;

  const { data: flashLoanHasExecutorRole } = useReadContract({
    abi: flashLoanExecutorAbi,
    address: flashLoanExecutor,
    functionName: "hasRole",
    args:
      !!arbExecutor && !!flashLoanExecutor
        ? [EXECUTOR_ROLE, arbExecutor]
        : undefined,
    query: {
      enabled: isSepolia && !!flashLoanExecutor && !!arbExecutor,
    },
  });

  const { writeContract, isPending } = useWriteContract({
    mutation: {
      onSuccess: () => {
        setTxStatus({ type: "success", message: "Transaction confirmed" });
      },
      onError: (err) => {
        setTxStatus({
          type: "error",
          message: err instanceof Error ? err.message : "Transaction failed",
        });
      },
    },
  });

  function setPending(label: string) {
    setTxStatus({ type: "pending", message: label });
  }

  function handleToggleToken(token: `0x${string}`, next: boolean) {
    if (!arbExecutor) return;
    setPending(`${next ? "Approving" : "Revoking"} token ${abbreviate(token)}`);
    writeContract({
      abi: arbitrageExecutorAbi,
      address: arbExecutor,
      functionName: "setTokenApproval",
      args: [token, next],
    });
  }

  function handleToggleRouter(next: boolean) {
    if (!arbExecutor) return;
    setPending(`${next ? "Approving" : "Revoking"} router ${abbreviate(UNI_V2_ROUTER)}`);
    writeContract({
      abi: arbitrageExecutorAbi,
      address: arbExecutor,
      functionName: "setRouterApproval",
      args: [UNI_V2_ROUTER as `0x${string}`, next],
    });
  }

  const allowanceManager = isAddressLike(allowanceManagerAddress)
    ? (allowanceManagerAddress as `0x${string}`)
    : undefined;

  function handleBatchGrantAllowance() {
    if (!allowanceManager || !arbExecutor) return;
    setPending("Granting allowances via AllowanceManager");
    writeContract({
      abi: allowanceManagerAbi,
      address: allowanceManager,
      functionName: "batchGrantAllowance",
      args: [
        [WETH as `0x${string}`, USDC as `0x${string}`],
        [arbExecutor, arbExecutor],
        [BigInt(MAX_ALLOWANCE), BigInt(MAX_ALLOWANCE)],
      ],
    });
  }

  function handleSetAllowanceManager() {
    if (!allowanceManager || !arbExecutor) return;
    setPending("Setting AllowanceManager on ArbitrageExecutor");
    writeContract({
      abi: arbitrageExecutorAbi,
      address: arbExecutor,
      functionName: "setAllowanceManager",
      args: [allowanceManager],
    });
  }

  if (!mounted) {
    return (
      <Card data-testid="contract-admin-panel" className="opacity-60">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Shield className="size-4" />
            Contract admin
          </CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">Connect wallet to load admin panel.</p>
        </CardContent>
      </Card>
    );
  }

  if (!isConnected) {
    return (
      <Card data-testid="contract-admin-panel">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Shield className="size-4" />
            Contract admin
          </CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">
            Wallet disconnected. Admin controls require a Sepolia connection.
          </p>
        </CardContent>
      </Card>
    );
  }

  if (!isSepolia) {
    return (
      <Card data-testid="contract-admin-panel">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Shield className="size-4" />
            Contract admin
          </CardTitle>
        </CardHeader>
        <CardContent>
          <Badge variant="warning">Wrong network</Badge>
          <p className="mt-2 text-sm text-muted-foreground">
            Admin controls are only available on Sepolia (chain ID {SEPOLIA_CHAIN_ID}).
          </p>
        </CardContent>
      </Card>
    );
  }

  const adminControlsEnabled = isAdmin === true;

  return (
    <Card data-testid="contract-admin-panel">
      <CardHeader>
        <CardTitle className="flex items-center justify-between">
          <span className="flex items-center gap-2">
            <Shield className="size-4" />
            Contract admin — Sepolia
          </span>
          <Badge variant={adminControlsEnabled ? "success" : "secondary"}>
            {isAdminLoading ? "checking…" : adminControlsEnabled ? "admin" : "not admin"}
          </Badge>
        </CardTitle>
      </CardHeader>

      <CardContent className="space-y-6">
        <div className="grid gap-4 sm:grid-cols-3">
          <div className="space-y-2">
            <Label htmlFor="arb-executor">ArbitrageExecutor</Label>
            <Input
              id="arb-executor"
              placeholder="0x..."
              value={arbExecutorAddress}
              onChange={(e) => setArbExecutorAddress(e.target.value)}
              disabled={isPending}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="flashloan-executor">FlashLoanExecutor</Label>
            <Input
              id="flashloan-executor"
              placeholder="0x..."
              value={flashLoanExecutorAddress}
              onChange={(e) => setFlashLoanExecutorAddress(e.target.value)}
              disabled={isPending}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="allowance-manager">AllowanceManager</Label>
            <Input
              id="allowance-manager"
              placeholder="0x..."
              value={allowanceManagerAddress}
              onChange={(e) => setAllowanceManagerAddress(e.target.value)}
              disabled={isPending}
            />
          </div>
        </div>

        {!!arbExecutor && (
          <div className="rounded-lg border bg-muted/30 p-4">
            <h4 className="mb-3 text-sm font-medium">Read-only state</h4>
            <dl className="grid grid-cols-1 gap-3 text-sm sm:grid-cols-2">
              <div className="flex items-center justify-between gap-2">
                <dt className="text-muted-foreground">WETH approved</dt>
                <dd>
                  <Badge variant={wethApproved ? "success" : "secondary"}>
                    {wethApproved ? "yes" : "no"}
                  </Badge>
                </dd>
              </div>
              <div className="flex items-center justify-between gap-2">
                <dt className="text-muted-foreground">USDC approved</dt>
                <dd>
                  <Badge variant={usdcApproved ? "success" : "secondary"}>
                    {usdcApproved ? "yes" : "no"}
                  </Badge>
                </dd>
              </div>
              <div className="flex items-center justify-between gap-2">
                <dt className="text-muted-foreground">Uniswap V2 router approved</dt>
                <dd>
                  <Badge variant={routerApproved ? "success" : "secondary"}>
                    {routerApproved ? "yes" : "no"}
                  </Badge>
                </dd>
              </div>
              <div className="flex items-center justify-between gap-2">
                <dt className="text-muted-foreground">AllowanceManager</dt>
                <dd className="font-mono text-xs">
                  {currentAllowanceManager &&
                  currentAllowanceManager !== "0x0000000000000000000000000000000000000000"
                    ? abbreviate(currentAllowanceManager)
                    : "none"}
                </dd>
              </div>
              <div className="flex items-center justify-between gap-2">
                <dt className="text-muted-foreground">FlashLoanExecutor EXECUTOR_ROLE</dt>
                <dd>
                  <Badge variant={flashLoanHasExecutorRole ? "success" : "secondary"}>
                    {flashLoanHasExecutorRole ? "granted" : "not granted"}
                  </Badge>
                </dd>
              </div>
            </dl>
          </div>
        )}

        {adminControlsEnabled && (
          <div className="space-y-6">
            <div className="space-y-3">
              <h4 className="text-sm font-medium">Token approvals</h4>
              <div className="flex items-center justify-between rounded-md border p-3">
                <div className="space-y-0.5">
                  <Label className="font-normal">WETH</Label>
                  <p className="text-xs font-mono text-muted-foreground">{WETH}</p>
                </div>
                <Switch
                  checked={!!wethApproved}
                  onCheckedChange={(checked) => handleToggleToken(WETH as `0x${string}`, checked)}
                  disabled={isPending}
                />
              </div>
              <div className="flex items-center justify-between rounded-md border p-3">
                <div className="space-y-0.5">
                  <Label className="font-normal">USDC</Label>
                  <p className="text-xs font-mono text-muted-foreground">{USDC}</p>
                </div>
                <Switch
                  checked={!!usdcApproved}
                  onCheckedChange={(checked) => handleToggleToken(USDC as `0x${string}`, checked)}
                  disabled={isPending}
                />
              </div>
            </div>

            <div className="space-y-3">
              <h4 className="text-sm font-medium">Router approvals</h4>
              <div className="flex items-center justify-between rounded-md border p-3">
                <div className="space-y-0.5">
                  <Label className="font-normal">Uniswap V2 Router</Label>
                  <p className="text-xs font-mono text-muted-foreground">{UNI_V2_ROUTER}</p>
                </div>
                <Switch
                  checked={!!routerApproved}
                  onCheckedChange={handleToggleRouter}
                  disabled={isPending}
                />
              </div>
            </div>

            <div className="space-y-3">
              <h4 className="text-sm font-medium">AllowanceManager configuration</h4>
              <div className="flex flex-wrap gap-2">
                <Button
                  variant="outline"
                  onClick={handleBatchGrantAllowance}
                  disabled={!allowanceManager || !arbExecutor || isPending}
                >
                  {isPending && txStatus.message?.startsWith("Granting") && (
                    <Loader2 className="mr-2 size-4 animate-spin" />
                  )}
                  batchGrantAllowance
                </Button>
                <Button
                  variant="outline"
                  onClick={handleSetAllowanceManager}
                  disabled={!allowanceManager || !arbExecutor || isPending}
                >
                  {isPending && txStatus.message?.startsWith("Setting") && (
                    <Loader2 className="mr-2 size-4 animate-spin" />
                  )}
                  setAllowanceManager
                </Button>
              </div>
            </div>
          </div>
        )}

        {txStatus.type !== "idle" && (
          <div
            className={cn(
              "flex items-start gap-2 rounded-md border p-3 text-sm",
              txStatus.type === "success" && "border-success/30 bg-success/10 text-success",
              txStatus.type === "error" && "border-destructive/30 bg-destructive/10 text-destructive",
              txStatus.type === "pending" && "border-muted bg-muted/50 text-muted-foreground",
            )}
          >
            {txStatus.type === "success" && <CheckCircle2 className="mt-0.5 size-4 shrink-0" />}
            {txStatus.type === "error" && <XCircle className="mt-0.5 size-4 shrink-0" />}
            {txStatus.type === "pending" && <Loader2 className="mt-0.5 size-4 shrink-0 animate-spin" />}
            <span>{txStatus.message}</span>
          </div>
        )}

        <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
          <Badge variant="outline">Read-only by default</Badge>
          <Badge variant="outline">Admin-only writes</Badge>
          <Badge variant="outline">No broadcast beyond config</Badge>
        </div>
      </CardContent>
    </Card>
  );
}

#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Deploy ArbitrageX v2 contracts to Sepolia + post-deploy config + smoke test.

.DESCRIPTION
    One-click pipeline:
    1. Validate environment (keys, SepoliaETH balance)
    2. Deploy contracts via forge script
    3. Extract proxy addresses from output
    4. Run post-deploy configuration (approvals, allowances, roles)
    5. Print environment variables for .env

.PARAMETER DeployerKey
    Private key of the deployer wallet (testnet only).

.PARAMETER MultisigAddress
    Address that will be timelock proposer/executor.

.PARAMETER RpcUrl
    Sepolia RPC endpoint (default: public node).

.EXAMPLE
    .\scripts\deploy-and-smoke-sepolia.ps1 -DeployerKey "0xabc..." -MultisigAddress "0xdef..."
#>

[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Mandatory = $true)]
    [string]$DeployerKey,

    [Parameter(Mandatory = $true)]
    [string]$MultisigAddress,

    [string]$RpcUrl = "https://ethereum-sepolia-rpc.publicnode.com"
)

$ErrorActionPreference = "Stop"

# ── Constants ──────────────────────────────────────────────────────────────
$SepoliaChainId = 11155111
$SepoliaWeth    = "0xfff9976782d46cc05630d1f6ebab18b2324d6b14"
$SepoliaUsdc    = "0x1c7d4b196cb0c7b01d743fbc6116a902379c7238"
$UniV2Router    = "0xeE567Fe1712Faf6149d80dA1E6934E354124CfE3"
$ProjectRoot    = Resolve-Path "$PSScriptRoot\.."
$ContractsDir   = Join-Path $ProjectRoot "contracts"
$OutFile        = Join-Path $env:TEMP "arbx-deploy-sepolia-$(Get-Date -Format yyyyMMddHHmmss).json"

# ── Helpers ────────────────────────────────────────────────────────────────
function Test-Address($addr) {
    return $addr -match "^0x[a-fA-F0-9]{40}$"
}

function Write-Step($n, $msg) {
    Write-Host "`n[STEP $n] $msg" -ForegroundColor Cyan
    Write-Host ("─" * 70) -ForegroundColor DarkGray
}

function Write-Success($msg) {
    Write-Host "  ✓ $msg" -ForegroundColor Green
}

function Write-Warn($msg) {
    Write-Host "  ⚠ $msg" -ForegroundColor Yellow
}

function Write-Error($msg) {
    Write-Host "  ✗ $msg" -ForegroundColor Red
}

function Invoke-Cast($argsList) {
    $proc = Start-Process -FilePath "cast" -ArgumentList $argsList `
        -WorkingDirectory $ContractsDir -PassThru -Wait -NoNewWindow
    if ($proc.ExitCode -ne 0) {
        throw "cast command failed with exit code $($proc.ExitCode)"
    }
}

# ── 0. Validate inputs ─────────────────────────────────────────────────────
Write-Step 0 "VALIDATE INPUTS"

if (-not (Test-Address $DeployerKey)) {
    if ($DeployerKey.Length -eq 64 -or $DeployerKey.Length -eq 66) {
        if (-not $DeployerKey.StartsWith("0x")) {
            $DeployerKey = "0x$DeployerKey"
        }
    } else {
        throw "DEPLOYER_PRIVATE_KEY must be a valid 0x-prefixed hex private key"
    }
}

if (-not (Test-Address $MultisigAddress)) {
    throw "MULTISIG_ADDRESS must be a valid Ethereum address"
}

Write-Success "Inputs validated"

# ── 1. Check forge/cast installed ──────────────────────────────────────────
Write-Step 1 "CHECK TOOLCHAIN"
try {
    $forgeVersion = (forge --version 2>$null)
    $castVersion = (cast --version 2>$null)
    Write-Success "forge: $forgeVersion"
    Write-Success "cast: $castVersion"
} catch {
    throw "Foundry not found. Install: https://book.getfoundry.sh/getting-started/installation"
}

# ── 2. Check SepoliaETH balance ────────────────────────────────────────────
Write-Step 2 "CHECK DEPLOYER BALANCE"
$deployerAddress = cast wallet address --private-key $DeployerKey
$balanceWei = cast balance $deployerAddress --rpc-url $RpcUrl
$balanceEth = [decimal]::Parse($balanceWei) / 1e18

Write-Host "  Deployer: $deployerAddress"
Write-Host "  Balance:  $balanceEth SepoliaETH"

if ($balanceEth -lt 0.05) {
    throw "Insufficient SepoliaETH. Get from https://sepolia-faucet.pk910.de (need >= 0.05)"
}
Write-Success "Balance sufficient"

# ── 3. Deploy contracts ────────────────────────────────────────────────────
Write-Step 3 "DEPLOY CONTRACTS TO SEPOLIA"
Write-Host "  This will broadcast 4 proxy deployments + role transfers."
Write-Host "  Est. gas: ~0.05 SepoliaETH"

if (-not $PSCmdlet.ShouldProcess("Sepolia testnet", "Deploy ArbitrageX v2 contracts")) {
    Write-Warn "Deploy skipped by user"
    exit 0
}

$env:DEPLOYER_PRIVATE_KEY       = $DeployerKey
$env:MULTISIG_ADDRESS           = $MultisigAddress
$env:CONFIRM_SEPOLIA_DEPLOY     = "true"
$env:SEPOLIA_RPC_URL            = $RpcUrl

Set-Location $ContractsDir

Write-Host "  Running forge script... (this may take 60-120s)"
$scriptOutput = forge script script/DeploySepolia.s.sol `
    --rpc-url $RpcUrl `
    --broadcast `
    -vvvv 2>&1

$scriptOutput | Out-File -FilePath "$env:TEMP\arbx-forge-output.log" -Encoding utf8

# ── 4. Extract proxy addresses ─────────────────────────────────────────────
Write-Step 4 "EXTRACT PROXY ADDRESSES"

$proxyAe = $scriptOutput | Select-String "ArbitrageExecutor proxy\s+:\s+(0x[a-fA-F0-9]{40})" | ForEach-Object { $_.Matches[0].Groups[1].Value }
$proxyAm = $scriptOutput | Select-String "AllowanceManager proxy\s+:\s+(0x[a-fA-F0-9]{40})" | ForEach-Object { $_.Matches[0].Groups[1].Value }
$proxyFl = $scriptOutput | Select-String "FlashLoanExecutor proxy\s+:\s+(0x[a-fA-F0-9]{40})" | ForEach-Object { $_.Matches[0].Groups[1].Value }
$proxyTl = $scriptOutput | Select-String "AdminTimelock proxy\s+:\s+(0x[a-fA-F0-9]{40})" | ForEach-Object { $_.Matches[0].Groups[1].Value }

if (-not $proxyAe -or -not $proxyAm -or -not $proxyFl -or -not $proxyTl) {
    Write-Error "Failed to extract all proxy addresses from forge output"
    Write-Host "  Check $env:TEMP\arbx-forge-output.log for details"
    throw "Deploy may have succeeded but parsing failed — inspect log manually"
}

Write-Success "ArbitrageExecutor : $proxyAe"
Write-Success "AllowanceManager  : $proxyAm"
Write-Success "FlashLoanExecutor : $proxyFl"
Write-Success "AdminTimelock     : $proxyTl"

# Save addresses to JSON
$deployInfo = @{
    chainId = $SepoliaChainId
    rpcUrl = $RpcUrl
    deployer = $deployerAddress
    timestamp = (Get-Date -Format o)
    proxies = @{
        ArbitrageExecutor = $proxyAe
        AllowanceManager = $proxyAm
        FlashLoanExecutor = $proxyFl
        AdminTimelock = $proxyTl
    }
}
$deployInfo | ConvertTo-Json -Depth 3 | Out-File $OutFile -Encoding utf8
Write-Success "Addresses saved to: $OutFile"

# ── 5. Post-deploy configuration ───────────────────────────────────────────
Write-Step 5 "POST-DEPLOY CONFIGURATION"

$castArgs = @("--rpc-url", $RpcUrl, "--private-key", $DeployerKey)

Write-Host "  5a. setAllowanceManager(...)"
Invoke-Cast (@("send", $proxyAe, "setAllowanceManager(address)", $proxyAm) + $castArgs)
Write-Success "AllowanceManager wired"

Write-Host "  5b. setTokenApproval(WETH, true)"
Invoke-Cast (@("send", $proxyAe, "setTokenApproval(address,bool)", $SepoliaWeth, "true") + $castArgs)
Write-Success "WETH approved"

Write-Host "  5c. setTokenApproval(USDC, true)"
Invoke-Cast (@("send", $proxyAe, "setTokenApproval(address,bool)", $SepoliaUsdc, "true") + $castArgs)
Write-Success "USDC approved"

Write-Host "  5d. setRouterApproval(UniV2, true)"
Invoke-Cast (@("send", $proxyAe, "setRouterApproval(address,bool)", $UniV2Router, "true") + $castArgs)
Write-Success "Router approved"

Write-Host "  5e. batchGrantAllowance(...)"
$tokensJson = "[$SepoliaWeth,$SepoliaUsdc]"
$spendersJson = "[$UniV2Router,$UniV2Router]"
$amountsJson = "[115792089237316195423570985008687907853269984665640564039457584007913129639935,115792089237316195423570985008687907853269984665640564039457584007913129639935]"
Invoke-Cast (@("send", $proxyAm, "batchGrantAllowance(address[],address[],uint256[])", $tokensJson, $spendersJson, $amountsJson) + $castArgs)
Write-Success "Allowances granted"

Write-Host "  5f. setBalancerVault(...)"
$balancerVault = "0xBA12222222228d8Ba445958a75a0704d566BF2C8"
Invoke-Cast (@("send", $proxyFl, "setBalancerVault(address)", $balancerVault) + $castArgs)
Write-Success "Balancer vault configured"

Write-Host "  5g. setReferralCode(0)"
Invoke-Cast (@("send", $proxyFl, "setReferralCode(uint16)", "0") + $castArgs)
Write-Success "Referral code disabled"

# ── 6. Environment variables ───────────────────────────────────────────────
Write-Step 6 "ENVIRONMENT VARIABLES FOR VPS .env"
Write-Host "  Copy these into your VPS .env file:"
Write-Host ""
Write-Host "    ARBITRAGE_EXECUTOR=$proxyAe"
Write-Host "    FLASHLOAN_EXECUTOR=$proxyFl"
Write-Host "    FLASHLOAN_EXECUTOR_11155111=$proxyFl"
Write-Host "    ALLOWANCE_MANAGER=$proxyAm"
Write-Host ""

# ── Done ───────────────────────────────────────────────────────────────────
Write-Step "✓" "DEPLOY PIPELINE COMPLETE"
Write-Host "  Deployment JSON: $OutFile"
Write-Host "  Forge log:       $env:TEMP\arbx-forge-output.log"
Write-Host ""
Write-Host "  Next steps:"
Write-Host "    1. SSH to VPS and add contract addresses to .env"
Write-Host "    2. Restart sim-ctl: docker compose up -d --force-recreate --no-deps sim-ctl"
Write-Host "    3. Open http://<VPS_IP>/live-readiness in browser"
Write-Host "    4. Connect MetaMask to Sepolia"
Write-Host "    5. Click 'Run Sepolia Smoke Test' on G-SIM-1 card"
Write-Host "    6. On PASS → set ARBX_SIMULATOR_V2_READY=true in .env"

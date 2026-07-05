Attribute VB_Name = "SyncRpcCatalog"
Option Explicit

' SyncRpcCatalog - auto-sync RPC Providers -> Chain Builder.
'
'  SyncRpcToChainBuilder  : idempotent, non-destructive. Reads the unique chain
'                           names in "RPC Providers" and APPENDS any missing one
'                           to "Chain Builder" (col A name, col C chain_id from
'                           ArbxBundleShipper.ChainIdFor). NEVER modifies or
'                           deletes existing Chain Builder rows - the operator's
'                           checkmarks (col B) and ids are preserved byte-for-byte.
'                           Polymorphic: any chain (mainnet/testnet) with a known
'                           chain_id gets the id; unknown chains still get a row
'                           (operator fills the id manually).
'
'  SeedRpcProviders       : idempotent seed of the curated public RPC catalog
'                           (Ankr/MeowRPC/officials + Sepolia/Amoy testnets).
'                           Skips (chain, provider) pairs already present.
'
'  Worksheet_Change wiring: the installer injects a one-line Worksheet_Change
'                           event into the "RPC Providers" sheet module that
'                           calls SyncRpcToChainBuilder on every col A-D edit.
'                           Events disabled during the sync so writes to Chain
'                           Builder don't recurse.
'
' Doctrina: NO toca paper_mode, NO toca .env, NO hace broadcast. Solo lee/escribe
' las hojas del workbook. El operador sigue siendo quien marca (col B) que chain
' se activa y quien corre ShipBundle.

Private Const RPC_SHEET As String = "RPC Providers"
Private Const CB_SHEET As String = "Chain Builder"
Private Const CB_DATA_START As Long = 4   ' col A header rows 1-3, data row 4+
Private Const RPC_DATA_START As Long = 2  ' col A header row 1, data row 2+

' ============================ SYNC: RPC -> Chain Builder ====================

' Idempotent: re-run is a no-op for chains already present. Non-destructive:
' only APPENDS new chain rows; never modifies col B (checkmark) or existing ids.
Public Sub SyncRpcToChainBuilder()
    Dim wsRpc As Worksheet, wsCb As Worksheet
    Dim rpcChains As Object, existing As Object
    Dim lastRow As Long, r As Long, nm As String, nextRow As Long, added As Long
    Dim cid As Variant, k As Variant

    On Error GoTo fail
    Set wsRpc = ThisWorkbook.Worksheets(RPC_SHEET)
    Set wsCb = ThisWorkbook.Worksheets(CB_SHEET)
    Set rpcChains = CreateObject("Scripting.Dictionary")
    Set existing = CreateObject("Scripting.Dictionary")

    ' 1. Collect unique chain names from RPC Providers (col A, data rows).
    lastRow = wsRpc.Cells(wsRpc.Rows.Count, 1).End(xlUp).Row
    For r = RPC_DATA_START To lastRow
        nm = Trim$(CStr(wsRpc.Cells(r, 1).Value))
        If Len(nm) > 0 And Not rpcChains.Exists(nm) Then rpcChains.Add nm, r
    Next r

    ' 2. Collect existing Chain Builder chain names (col A, from row 4).
    Dim cbLastRow As Long
    cbLastRow = wsCb.Cells(wsCb.Rows.Count, 1).End(xlUp).Row
    If cbLastRow < CB_DATA_START - 1 Then cbLastRow = CB_DATA_START - 1
    For r = CB_DATA_START To cbLastRow
        nm = Trim$(CStr(wsCb.Cells(r, 1).Value))
        If Len(nm) > 0 And Not existing.Exists(nm) Then existing.Add nm, r
    Next r

    ' 3. Append missing chains (only new rows - existing rows untouched).
    nextRow = cbLastRow + 1
    For Each k In rpcChains.Keys
        If Not existing.Exists(CStr(k)) Then
            wsCb.Cells(nextRow, 1).Value = CStr(k)
            ' chain_id from the canonical map (Private->Public in ArbxBundleShipper).
            cid = ChainIdFor(CStr(k))
            If Not IsEmpty(cid) Then wsCb.Cells(nextRow, 3).Value = CLng(cid)
            ' col B (checkmark) left blank - operator chooses what to activate.
            existing.Add CStr(k), nextRow
            nextRow = nextRow + 1
            added = added + 1
        End If
    Next k

    Application.StatusBar = "SyncRpcToChainBuilder: +" & added & " chain(s) appended."
    Exit Sub
fail:
    Application.StatusBar = "SyncRpcToChainBuilder FAILED: " & Err.Description
End Sub

' ============================ SEED: curated public RPC list =================

' Idempotent seed of the curated catalog. Skips (chain, provider) pairs already
' present so re-running after editing the list only adds the new entries.
Public Sub SeedRpcProviders()
    Dim ws As Worksheet, existing As Object
    Dim lastRow As Long, r As Long, key As String, added As Long, nextRow As Long
    Dim seed As Collection, entry As Variant

    On Error GoTo fail
    Set ws = ThisWorkbook.Worksheets(RPC_SHEET)
    Set existing = CreateObject("Scripting.Dictionary")

    ' 1. Index existing (chain|provider) so the seed is idempotent.
    lastRow = ws.Cells(ws.Rows.Count, 1).End(xlUp).Row
    For r = RPC_DATA_START To lastRow
        key = LCase$(Trim$(CStr(ws.Cells(r, 1).Value))) & "|" & LCase$(Trim$(CStr(ws.Cells(r, 3).Value)))
        If Len(key) > 1 Then existing(key) = r
    Next r

    ' 2. The curated catalog (chain, protocol, provider, url). chainlist.org verified.
    '    Each Add is a standalone statement - avoids the VBA 25-line-continuation limit.
    Set seed = New Collection
    seed.Add Array("Ethereum Mainnet", "HTTP", "Ankr", "https://rpc.ankr.com/eth")
    seed.Add Array("Ethereum Mainnet", "HTTP", "MeowRPC", "https://eth.meowrpc.com")
    seed.Add Array("Ethereum Mainnet", "WSS", "Ankr", "wss://rpc.ankr.com/eth/ws")
    seed.Add Array("Ethereum Mainnet", "WSS", "BlockPI", "wss://ethereum.public.blockpi.network/v1/ws/public")
    seed.Add Array("Ethereum Sepolia", "HTTP", "Ankr", "https://rpc.ankr.com/eth_sepolia")
    seed.Add Array("Ethereum Sepolia", "HTTP", "Sepolia.org", "https://rpc.sepolia.org")
    seed.Add Array("Ethereum Sepolia", "WSS", "Ankr", "wss://rpc.ankr.com/eth_sepolia/ws")
    seed.Add Array("Ethereum Holesky", "HTTP", "Ankr", "https://rpc.ankr.com/eth_holesky")
    seed.Add Array("Ethereum Holesky", "HTTP", "Ethpandaops", "https://rpc.holesky.ethpandaops.io")
    seed.Add Array("BSC Mainnet", "HTTP", "Ankr", "https://rpc.ankr.com/bsc")
    seed.Add Array("BSC Mainnet", "HTTP", "Binance", "https://bsc-dataseed.binance.org/")
    seed.Add Array("BSC Mainnet", "HTTP", "MeowRPC", "https://bsc.meowrpc.com")
    seed.Add Array("BSC Mainnet", "WSS", "Ankr", "wss://rpc.ankr.com/bsc/ws")
    seed.Add Array("Polygon Mainnet", "HTTP", "Ankr", "https://rpc.ankr.com/polygon")
    seed.Add Array("Polygon Mainnet", "HTTP", "Polygon Labs", "https://polygon-rpc.com/")
    seed.Add Array("Polygon Mainnet", "HTTP", "MeowRPC", "https://polygon.meowrpc.com")
    seed.Add Array("Polygon Mainnet", "WSS", "Ankr", "wss://rpc.ankr.com/polygon/ws")
    seed.Add Array("Polygon Amoy", "HTTP", "Polygon Labs", "https://rpc-amoy.polygon.technology")
    seed.Add Array("Arbitrum One", "HTTP", "Ankr", "https://rpc.ankr.com/arbitrum")
    seed.Add Array("Arbitrum One", "HTTP", "Arbitrum Foundation", "https://arb1.arbitrum.io/rpc")
    seed.Add Array("Arbitrum One", "HTTP", "MeowRPC", "https://arbitrum.meowrpc.com")
    seed.Add Array("Arbitrum One", "WSS", "Ankr", "wss://rpc.ankr.com/arbitrum/ws")
    seed.Add Array("Arbitrum Sepolia", "HTTP", "Arbitrum Foundation", "https://sepolia-rollup.arbitrum.io/rpc")
    seed.Add Array("Arbitrum Sepolia", "HTTP", "Ankr", "https://rpc.ankr.com/arbitrum_sepolia")
    seed.Add Array("Optimism", "HTTP", "Ankr", "https://rpc.ankr.com/optimism")
    seed.Add Array("Optimism", "HTTP", "OP Labs", "https://mainnet.optimism.io")
    seed.Add Array("Optimism", "WSS", "Ankr", "wss://rpc.ankr.com/optimism/ws")
    seed.Add Array("Optimism Sepolia", "HTTP", "OP Labs", "https://sepolia.optimism.io")
    seed.Add Array("Optimism Sepolia", "HTTP", "Ankr", "https://rpc.ankr.com/optimism_sepolia")
    seed.Add Array("Base", "HTTP", "Ankr", "https://rpc.ankr.com/base")
    seed.Add Array("Base", "HTTP", "Base/Coinbase", "https://mainnet.base.org")
    seed.Add Array("Base", "WSS", "Ankr", "wss://rpc.ankr.com/base/ws")
    seed.Add Array("Base Sepolia", "HTTP", "Base", "https://sepolia.base.org")
    seed.Add Array("Base Sepolia", "HTTP", "Ankr", "https://rpc.ankr.com/base_sepolia")
    seed.Add Array("Avalanche", "HTTP", "Ankr", "https://rpc.ankr.com/avalanche")
    seed.Add Array("Avalanche", "HTTP", "Ava Labs", "https://api.avax.network/ext/bc/C/rpc")
    seed.Add Array("Avalanche", "WSS", "Ankr", "wss://rpc.ankr.com/avalanche/ws")
    seed.Add Array("Gnosis", "HTTP", "Ankr", "https://rpc.ankr.com/gnosis")
    seed.Add Array("Gnosis", "HTTP", "Gnosis Chain", "https://rpc.gnosischain.com")
    seed.Add Array("Gnosis", "WSS", "Ankr", "wss://rpc.ankr.com/gnosis/ws")
    seed.Add Array("Linea", "HTTP", "Linea", "https://rpc.linea.build")
    seed.Add Array("Linea", "HTTP", "Ankr", "https://rpc.ankr.com/linea")
    seed.Add Array("Scroll", "HTTP", "Scroll", "https://rpc.scroll.io")
    seed.Add Array("Scroll", "HTTP", "Ankr", "https://rpc.ankr.com/scroll")
    seed.Add Array("Blast", "HTTP", "Blast", "https://rpc.blast.io")
    seed.Add Array("Blast", "HTTP", "Ankr", "https://rpc.ankr.com/blast")

    ' 3. Append only the (chain, provider) pairs not already present.
    nextRow = lastRow + 1
    For Each entry In seed
        key = LCase$(CStr(entry(0))) & "|" & LCase$(CStr(entry(2)))
        If Not existing.Exists(key) Then
            ws.Cells(nextRow, 1).Value = CStr(entry(0))
            ws.Cells(nextRow, 2).Value = CStr(entry(1))
            ws.Cells(nextRow, 3).Value = CStr(entry(2))
            ws.Cells(nextRow, 4).Value = CStr(entry(3))
            existing(key) = nextRow
            nextRow = nextRow + 1
            added = added + 1
        End If
    Next entry

    ' 4. After seeding, sync the new chains into Chain Builder.
    SyncRpcToChainBuilder
    Application.StatusBar = "SeedRpcProviders: +" & added & " RPC row(s) appended."
    Exit Sub
fail:
    Application.StatusBar = "SeedRpcProviders FAILED: " & Err.Description
End Sub

' Manual one-shot: seed the catalog + sync + report (operator runs via Alt+F8
' or a button; the installer also calls this once on install).
Public Sub InstallSeedAndSync()
    Dim r As VbMsgBoxResult
    r = MsgBox("Sembrar el catalogo RPC curado (45 endpoints publicos) en 'RPC Providers'" & _
               " y sync-ar las chains nuevas a 'Chain Builder'?" & vbCrLf & vbCrLf & _
               "Idempotente: solo anade lo que no exista. No toca tus filas actuales.", _
               vbQuestion + vbYesNo + vbDefaultButton1, "ArbX RPC catalog seed")
    If r = vbYes Then
        SeedRpcProviders
        MsgBox "Listo. Revisa 'RPC Providers' (filas nuevas) y 'Chain Builder' (chains nuevas)." & vbCrLf & _
               "Marca las que quieras activar en col B de Chain Builder.", vbInformation, "ArbX"
    End If
End Sub

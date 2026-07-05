Attribute VB_Name = "SyncRpcCatalog"
Option Explicit

' SyncRpcCatalog - auto-sync RPC Providers -> Chain Builder + full matrix rebuild.
'
' NON-DESTRUCTIVE: preserves the operator's tracking columns (whatever they added
' beyond col H) on existing matrix rows. Coverage columns (Flash Loan / Flash Swap /
' Lending / Collateral-free) land in DYNAMIC columns - the first free block AFTER
' the operator's existing headers - never overwriting their data.
'
'  SyncRpcToChainBuilder : LIGHT, idempotent. Called by Worksheet_Change. Inserts
'                          missing chains CONTIGUOUS with the matrix (shifts the
'                          footer down), formatted, all cols filled + coverage.
'  RebuildChainBuilder   : HEAVY, manual. Full clean rebuild: existing matrix rows
'                          (all cols preserved) + new chains (computed cols) +
'                          coverage cols at the dynamic offset. Dedup's stale rows.
'  SeedRpcProviders      : idempotent seed of the curated public RPC catalog.

Private Const RPC_SHEET As String = "RPC Providers"
Private Const CB_SHEET As String = "Chain Builder"
Private Const CB_DATA_START As Long = 4
Private Const RPC_DATA_START As Long = 2

' ============================ LIGHT SYNC (Worksheet_Change) =================

Public Sub SyncRpcToChainBuilder()
    Dim wsCb As Worksheet, wsRpc As Worksheet
    Dim existing As Object, rpcOrdered As Object
    Dim k As Variant, matrixEnd As Long, covStart As Long, maxCol As Long, added As Long

    On Error GoTo fail
    Set wsCb = ThisWorkbook.Worksheets(CB_SHEET)
    Set wsRpc = ThisWorkbook.Worksheets(RPC_SHEET)
    Set existing = CreateObject("Scripting.Dictionary")
    Set rpcOrdered = CreateObject("Scripting.Dictionary")

    Application.EnableEvents = False
    Application.Calculation = xlCalculationManual

    matrixEnd = MatrixEndRow(wsCb)
    maxCol = LastHeaderCol(wsCb)
    covStart = CoverageStartCol(wsCb, maxCol)
    EnsureCoverageHeaders wsCb, covStart

    Dim r As Long
    For r = CB_DATA_START To matrixEnd
        Dim nm As String
        nm = Trim$(CStr(wsCb.Cells(r, 1).Value))
        If Len(nm) > 0 And Not existing.Exists(nm) Then existing.Add nm, r
    Next r

    Dim lastRpc As Long
    lastRpc = wsRpc.Cells(wsRpc.Rows.Count, 1).End(xlUp).Row
    For r = RPC_DATA_START To lastRpc
        nm = Trim$(CStr(wsRpc.Cells(r, 1).Value))
        If Len(nm) > 0 And Not rpcOrdered.Exists(nm) Then rpcOrdered.Add nm, r
    Next r

    Dim missing As Collection
    Set missing = New Collection
    For Each k In rpcOrdered.Keys
        If Not existing.Exists(CStr(k)) Then missing.Add CStr(k)
    Next k
    If missing.Count = 0 Then GoTo done

    ' Insert rows AFTER matrixEnd so new chains stay contiguous (footer shifts down).
    wsCb.Rows(matrixEnd + 1 & ":" & matrixEnd + missing.Count).Insert Shift:=xlShiftDown
    Dim outRow As Long, chainName As String, cid As Variant
    outRow = matrixEnd + 1
    For Each k In missing
        chainName = CStr(k)
        cid = ChainIdFor(chainName)
        WriteChainRow wsCb, outRow, chainName, cid, covStart, True
        If matrixEnd >= CB_DATA_START Then
            wsCb.Rows(matrixEnd).Copy
            wsCb.Rows(outRow).PasteSpecial Paste:=xlPasteFormats
            Application.CutCopyMode = False
        End If
        outRow = outRow + 1
        added = added + 1
    Next k

done:
    Application.Calculation = xlCalculationAutomatic
    Application.EnableEvents = True
    Application.StatusBar = "SyncRpcToChainBuilder: +" & added & " chain(s) at row " & (matrixEnd + 1)
    Exit Sub
fail:
    Application.Calculation = xlCalculationAutomatic
    Application.EnableEvents = True
    Application.StatusBar = "SyncRpcToChainBuilder FAILED: " & Err.Description
End Sub

' ============================ HEAVY REBUILD (manual) =======================

' Non-destructive full rebuild. Existing matrix rows: ALL cols preserved (operator's
' tracking cols untouched). New chains: A-H computed + coverage. Coverage lands at
' the dynamic covStart (first free col after operator's headers). Footer text rows
' preserved; stale chain-name rows in the footer area are dropped (re-added from RPC).
Public Sub RebuildChainBuilder()
    Dim wsCb As Worksheet, wsRpc As Worksheet
    Dim matrixEnd As Long, maxRow As Long, maxCol As Long, covStart As Long, r As Long, c As Long

    On Error GoTo fail
    Set wsCb = ThisWorkbook.Worksheets(CB_SHEET)
    Set wsRpc = ThisWorkbook.Worksheets(RPC_SHEET)
    Application.EnableEvents = False
    Application.Calculation = xlCalculationManual

    matrixEnd = MatrixEndRow(wsCb)
    maxCol = LastHeaderCol(wsCb)
    covStart = CoverageStartCol(wsCb, maxCol)
    If covStart + 3 > maxCol Then maxCol = covStart + 3

    ' 1. Snapshot existing matrix rows - ALL cols 1..maxCol (preserve operator data).
    Dim existing As Collection: Set existing = New Collection
    Dim existingNames As Object: Set existingNames = CreateObject("Scripting.Dictionary")
    For r = CB_DATA_START To matrixEnd
        Dim nm As String
        nm = Trim$(CStr(wsCb.Cells(r, 1).Value))
        If Len(nm) = 0 Then Exit For
        Dim rd() As Variant: ReDim rd(1 To maxCol)
        For c = 1 To maxCol
            rd(c) = wsCb.Cells(r, c).Value
        Next c
        existing.Add Array(nm, rd)
        existingNames(nm) = existing.Count
    Next r

    ' 2. Snapshot TEXT footer only (rows after matrix whose col A is NOT a chain in
    '    RPC + has no numeric chain_id = instructions/notes). Stale chain-name rows
    '    are dropped here - they get re-added cleanly from RPC below.
    Dim footer As Collection: Set footer = New Collection
    Dim knownChainNames As Object: Set knownChainNames = CreateObject("Scripting.Dictionary")
    Dim lastRpc As Long: lastRpc = wsRpc.Cells(wsRpc.Rows.Count, 1).End(xlUp).Row
    Dim rr As Long
    For rr = RPC_DATA_START To lastRpc
        nm = Trim$(CStr(wsRpc.Cells(rr, 1).Value))
        If Len(nm) > 0 Then knownChainNames(LCase$(nm)) = rr
    Next rr
    maxRow = wsCb.Cells(wsCb.Rows.Count, 1).End(xlUp).Row
    For r = matrixEnd + 1 To maxRow
        Dim txt As String
        txt = Trim$(CStr(wsCb.Cells(r, 1).Value))
        If Len(txt) = 0 Then GoTo nextFooterRow
        Dim cidCheck As Variant: cidCheck = wsCb.Cells(r, 3).Value
        Dim isChainRow As Boolean: isChainRow = IsNumeric(cidCheck) Or knownChainNames.Exists(LCase$(txt))
        If Not isChainRow Then footer.Add txt   ' prose: instructions, notes
nextFooterRow:
    Next r

    ' 3. Ordered unique chains from RPC Providers -> the new-chains set (not existing).
    Dim rpcOrdered As Object: Set rpcOrdered = CreateObject("Scripting.Dictionary")
    For rr = RPC_DATA_START To lastRpc
        nm = Trim$(CStr(wsRpc.Cells(rr, 1).Value))
        If Len(nm) > 0 And Not rpcOrdered.Exists(nm) Then rpcOrdered.Add nm, rr
    Next rr
    Dim newChains As Collection: Set newChains = New Collection
    Dim k As Variant
    For Each k In rpcOrdered.Keys
        If Not existingNames.Exists(CStr(k)) Then newChains.Add CStr(k)
    Next k

    ' 4. Clear data area cols 1..maxCol rows 4..maxRow (we have the snapshots).
    If maxRow >= CB_DATA_START Then
        wsCb.Range(wsCb.Cells(CB_DATA_START, 1), wsCb.Cells(maxRow, maxCol)).ClearContents
    End If
    EnsureCoverageHeaders wsCb, covStart

    ' 5. Write existing matrix rows - restore ALL cols (operator data preserved) + coverage.
    Dim outRow As Long: outRow = CB_DATA_START
    Dim entry As Variant, cid As Variant, chainName As String
    For Each entry In existing
        chainName = CStr(entry(0))
        Dim rd2 As Variant: rd2 = entry(1)
        For c = 1 To UBound(rd2)
            If Not IsEmpty(rd2(c)) Then wsCb.Cells(outRow, c).Value = rd2(c)
        Next c
        cid = wsCb.Cells(outRow, 3).Value
        If IsNumeric(cid) Then WriteCoverageAt wsCb, outRow, CLng(cid), covStart
        outRow = outRow + 1
    Next entry

    ' 6. Write new chains (A-H computed + coverage). Format copied from row 4 template.
    Dim newStart As Long: newStart = outRow
    For Each k In newChains
        chainName = CStr(k)
        cid = ChainIdFor(chainName)
        WriteChainRow wsCb, outRow, chainName, cid, covStart, True
        outRow = outRow + 1
    Next k
    If newStart < outRow And existing.Count > 0 Then
        wsCb.Rows(CB_DATA_START).Copy
        wsCb.Rows(newStart & ":" & outRow - 1).PasteSpecial Paste:=xlPasteFormats
        Application.CutCopyMode = False
    End If

    ' 7. Footer: ALWAYS ensure the standard instructions + privacy rows are present
    '    at the bottom (idempotent). The collected-text-footer path is fragile when
    '    Worksheet_Change fires during SeedRpcProviders and shifts things, so we
    '    guarantee the help text is there. ChrW$ for the unicode so the .bas import
    '    (ANSI) doesn't mangle the checkmark / middot / em-dash.
    Dim fRow As Long: fRow = outRow + 1
    For Each k In footer
        wsCb.Cells(fRow, 1).Value = CStr(k)
        fRow = fRow + 1
    Next k
    Dim chk As String, midDot As String, emDash As String
    chk = ChrW$(&H2713)        ' check mark
    midDot = ChrW$(&H2219)     ' middle dot
    emDash = ChrW$(&H2014)     ' em dash
    Dim stdFooter As Variant
    stdFooter = Array( _
        "INSTRUCCIONES:  1) marca " & chk & " en col B de la chain a activar    " & _
        "2) python scripts/arbx-env-deploy/gen_chain_env.py --out <dir>    " & _
        "3) revisa fragments generados    " & _
        "4) RunFullSyncCycle sube al VPS (shred, no-print, paper_mode jam" & midDot & "s se toca)", _
        "PRIVACIDAD: RPC multi-provider CSV con SHUFFLE de orden por run " & emDash & _
        " el primer provider no fingerprinta la infra.")
    ' Idempotent: only append each standard line if its first 30 chars aren't already on the sheet.
    Dim fi As Long, existingText As String, checkR As Long
    existingText = ""
    For checkR = outRow + 1 To fRow - 1
        existingText = existingText & CStr(wsCb.Cells(checkR, 1).Value)
    Next checkR
    For fi = LBound(stdFooter) To UBound(stdFooter)
        If InStr(1, existingText, Left$(CStr(stdFooter(fi)), 30), vbTextCompare) = 0 Then
            wsCb.Cells(fRow, 1).Value = CStr(stdFooter(fi))
            existingText = existingText & CStr(stdFooter(fi))
            fRow = fRow + 1
        End If
    Next fi

    wsCb.Columns.AutoFit
    Application.Calculation = xlCalculationAutomatic
    Application.EnableEvents = True
    Application.StatusBar = "RebuildChainBuilder: " & existing.Count & " preserved + " & newChains.Count & " new"
    Exit Sub
fail:
    Application.Calculation = xlCalculationAutomatic
    Application.EnableEvents = True
    Application.StatusBar = "RebuildChainBuilder FAILED: " & Err.Description
End Sub

' ============================ coverage col detection =======================

' Last non-blank col in the header row (row 3). The operator's column footprint.
Private Function LastHeaderCol(ws As Worksheet) As Long
    Dim c As Long, last As Long
    last = 8  ' floor: A-H is always ours
    For c = 1 To 40
        If Len(Trim$(CStr(ws.Cells(3, c).Value))) > 0 Then last = c
    Next c
    LastHeaderCol = last
End Function

' Coverage lands here. Reuse the existing "Flash Loan" col (idempotent across runs),
' else the first free col after the operator's headers.
Private Function CoverageStartCol(ws As Worksheet, lastHdr As Long) As Long
    Dim c As Long
    For c = 9 To 40
        If StrComp(Trim$(CStr(ws.Cells(3, c).Value)), "Flash Loan", vbTextCompare) = 0 Then
            CoverageStartCol = c
            Exit Function
        End If
    Next c
    CoverageStartCol = lastHdr + 1
End Function

Private Sub EnsureCoverageHeaders(ws As Worksheet, covStart As Long)
    Dim hdrs As Variant
    hdrs = Array("Flash Loan", "Flash Swap", "Lending", "Collateral-free (same-block)")
    Dim i As Long
    For i = 0 To 3
        If Len(Trim$(CStr(ws.Cells(3, covStart + i).Value))) = 0 Then
            ws.Cells(3, covStart + i).Value = hdrs(i)
        End If
    Next i
End Sub

' ============================ row writers ==================================

Private Sub WriteChainRow(ws As Worksheet, row As Long, chainName As String, _
                          cid As Variant, covStart As Long, fillMeta As Boolean)
    ws.Cells(row, 1).Value = chainName
    If Not IsEmpty(cid) Then
        ws.Cells(row, 3).Value = CLng(cid)
        Dim nat As String, expl As String, bms As Long
        ChainMetaFull CLng(cid), nat, expl, bms
        If fillMeta Then
            ws.Cells(row, 4).Value = nat
            ws.Cells(row, 5).Value = expl
            ws.Cells(row, 6).Value = bms
        End If
        Dim hSum As String, wSum As String
        RpcSummary chainName, hSum, wSum
        If fillMeta Then
            ws.Cells(row, 7).Value = hSum
            ws.Cells(row, 8).Value = wSum
        End If
        WriteCoverageAt ws, row, CLng(cid), covStart
    Else
        If fillMeta Then
            Dim h2 As String, w2 As String
            RpcSummary chainName, h2, w2
            ws.Cells(row, 7).Value = h2
            ws.Cells(row, 8).Value = w2
        End If
    End If
End Sub

Private Sub WriteCoverageAt(ws As Worksheet, row As Long, cid As Long, covStart As Long)
    Dim cov As Variant
    cov = CoverageFor(cid)
    ws.Cells(row, covStart).Value = CStr(cov(0))
    ws.Cells(row, covStart + 1).Value = CStr(cov(1))
    ws.Cells(row, covStart + 2).Value = CStr(cov(2))
    ws.Cells(row, covStart + 3).Value = CStr(cov(3))
End Sub

Private Function CoverageFor(cid As Long) As Variant
    Select Case cid
        Case 1:      CoverageFor = Array("Si (Aave V3, DyDx, Maker)", "Si (Uniswap V3, Balancer)", "Si (Aave, Compound, Maker)", "Si (Aave flash)")
        Case 10:     CoverageFor = Array("Si (Aave V3)", "Si (Uniswap V3, Balancer)", "Si (Aave, Sonne, Extra)", "Si (Aave flash)")
        Case 56:     CoverageFor = Array("Si (PancakeSwap)", "Si (PancakeSwap, BiSwap)", "Si (Venus, Mars Protocol)", "Si (Venus)")
        Case 137:    CoverageFor = Array("Si (Aave V3)", "Si (Uniswap V3, Balancer, QuickSwap)", "Si (Aave, Compound)", "Si (Aave flash)")
        Case 8453:   CoverageFor = Array("Si (Aave V3, Seamless)", "Si (Uniswap V3, Aerodrome)", "Si (Aave, Seamless, Moonwell)", "Si (Aave flash)")
        Case 42161:  CoverageFor = Array("Si (Aave V3)", "Si (Uniswap V3, Balancer, Camelot)", "Si (Aave, Radiant, Compound V3)", "Si (Aave flash)")
        Case 43114:  CoverageFor = Array("Si (Aave V3)", "Si (Trader Joe, Balancer)", "Si (Aave, Benqi, Trader Joe)", "Si (Aave flash)")
        Case 100:    CoverageFor = Array("Parcial (Agave)", "Si (Honeyswap, SushiSwap)", "Si (Agave)", "Parcial")
        Case 59144:  CoverageFor = Array("Parcial", "Si (Sushi, Lynx)", "Parcial", "Parcial")
        Case 534352: CoverageFor = Array("Si (Aave V3)", "Si (SyncSwap, Skygate)", "Si (Aave V3)", "Si (Aave flash)")
        Case 81457:  CoverageFor = Array("Parcial", "Si (Thruster, Sushi)", "Si (Blast Lend)", "Parcial")
        Case 11155111: CoverageFor = Array("Si (Aave V3 sepolia)", "Si (Uniswap V3 sepolia)", "Si (Aave V3 sepolia)", "Si")
        Case 421614:   CoverageFor = Array("Si (Aave testnet)", "Parcial (Uniswap testnet)", "Si (Aave testnet)", "Si")
        Case 84532:    CoverageFor = Array("Si (Aave testnet)", "Parcial (Uniswap testnet)", "Si (Aave testnet)", "Si")
        Case 11155420: CoverageFor = Array("Parcial", "Parcial", "Parcial", "Parcial")
        Case 80002:    CoverageFor = Array("Parcial", "Parcial", "Parcial", "Parcial")
        Case 17000:    CoverageFor = Array("Parcial (deprecated)", "No", "Parcial", "Parcial")
        Case Else:     CoverageFor = Array("?", "?", "?", "?")
    End Select
End Function

' ============================ helpers ======================================

Private Function MatrixEndRow(ws As Worksheet) As Long
    Dim r As Long, lastRow As Long, matrixEnd As Long
    lastRow = ws.Cells(ws.Rows.Count, 1).End(xlUp).Row
    matrixEnd = CB_DATA_START - 1
    For r = CB_DATA_START To lastRow
        Dim nm As String, cid As Variant
        nm = Trim$(CStr(ws.Cells(r, 1).Value))
        cid = ws.Cells(r, 3).Value
        If Len(nm) = 0 Then Exit For
        If Not IsNumeric(cid) Then Exit For
        matrixEnd = r
    Next r
    MatrixEndRow = matrixEnd
End Function

Private Sub RpcSummary(chainName As String, ByRef httpSummary As String, ByRef wsSummary As String)
    Dim ws As Worksheet, r As Long, lastRow As Long
    Dim httpCt As Long, wsCt As Long, httpProvs As String, wsProvs As String
    Set ws = ThisWorkbook.Worksheets(RPC_SHEET)
    lastRow = ws.Cells(ws.Rows.Count, 1).End(xlUp).Row
    For r = RPC_DATA_START To lastRow
        If StrComp(Trim$(CStr(ws.Cells(r, 1).Value)), chainName, vbTextCompare) = 0 Then
            Dim proto As String, prov As String
            proto = UCase$(Trim$(CStr(ws.Cells(r, 2).Value)))
            prov = Trim$(CStr(ws.Cells(r, 3).Value))
            If Len(prov) = 0 Then GoTo nxt
            If proto = "HTTP" Then
                httpCt = httpCt + 1
                If httpCt <= 5 Then httpProvs = httpProvs & IIf(httpCt > 1, ", ", "") & prov
            ElseIf proto = "WSS" Or proto = "WS" Then
                wsCt = wsCt + 1
                If wsCt <= 5 Then wsProvs = wsProvs & IIf(wsCt > 1, ", ", "") & prov
            End If
        End If
nxt:
    Next r
    httpSummary = IIf(httpCt > 0, httpCt & " providers: " & httpProvs, "")
    wsSummary = IIf(wsCt > 0, wsCt & " providers: " & wsProvs, "")
End Sub

Private Sub ChainMetaFull(cid As Long, ByRef nat As String, ByRef expl As String, ByRef bms As Long)
    Select Case cid
        Case 1:       nat = "ETH":   expl = "https://etherscan.io":              bms = 12000
        Case 10:      nat = "ETH":   expl = "https://optimistic.etherscan.io":   bms = 2000
        Case 56:      nat = "BNB":   expl = "https://bscscan.com":               bms = 3000
        Case 100:     nat = "xDAI":  expl = "https://gnosisscan.io":             bms = 5000
        Case 137:     nat = "MATIC": expl = "https://polygonscan.com":           bms = 2000
        Case 8453:    nat = "ETH":   expl = "https://basescan.org":              bms = 2000
        Case 42161:   nat = "ETH":   expl = "https://arbiscan.io":               bms = 250
        Case 43114:   nat = "AVAX":  expl = "https://snowtrace.io":              bms = 2000
        Case 59144:   nat = "ETH":   expl = "https://lineascan.build":           bms = 12000
        Case 81457:   nat = "ETH":   expl = "https://blastscan.io":              bms = 2000
        Case 534352:  nat = "ETH":   expl = "https://scrollscan.com":            bms = 3000
        Case 11155111:nat = "ETH":   expl = "https://sepolia.etherscan.io":      bms = 12000
        Case 17000:   nat = "ETH":   expl = "https://holesky.etherscan.io":      bms = 12000
        Case 80002:   nat = "MATIC": expl = "https://amoy.polygonscan.com":      bms = 2000
        Case 421614:  nat = "ETH":   expl = "https://sepolia.arbiscan.io":       bms = 250
        Case 11155420:nat = "ETH":   expl = "https://optimism-sepolia.blockscout.com": bms = 2000
        Case 84532:   nat = "ETH":   expl = "https://sepolia.basescan.org":      bms = 2000
        Case Else:    nat = "":      expl = "":                                  bms = 12000
    End Select
End Sub

' ============================ SEED: curated public RPC list =================

Public Sub SeedRpcProviders()
    Dim ws As Worksheet, existing As Object
    Dim lastRow As Long, r As Long, key As String, added As Long, nextRow As Long
    Dim seed As Collection, entry As Variant

    On Error GoTo fail
    Set ws = ThisWorkbook.Worksheets(RPC_SHEET)
    Set existing = CreateObject("Scripting.Dictionary")

    lastRow = ws.Cells(ws.Rows.Count, 1).End(xlUp).Row
    For r = RPC_DATA_START To lastRow
        key = LCase$(Trim$(CStr(ws.Cells(r, 1).Value))) & "|" & LCase$(Trim$(CStr(ws.Cells(r, 3).Value)))
        If Len(key) > 1 Then existing(key) = r
    Next r

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

    Application.StatusBar = "SeedRpcProviders: +" & added & " RPC row(s) appended."
    Exit Sub
fail:
    Application.StatusBar = "SeedRpcProviders FAILED: " & Err.Description
End Sub

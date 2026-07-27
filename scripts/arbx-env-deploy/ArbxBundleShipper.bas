Attribute VB_Name = "ArbxBundleShipper"
Option Explicit

' ArbxBundleShipper - the Excel macro that GENERATES the encrypted config bundle.
'
'  ShipBundle  -> EL GENERADOR EN 1 CLIC:
'        1) Lee las 4 hojas (.env Production, RPC Providers, Tokens & Keys, Chain Builder)
'        2) Filtra NEVER_SHIP (layer 1 de 3: VBA -> Python -> Rust importer)
'        3) Serializa el bundle JSON a un temp
'        4) Shell 'pythonw encrypt_and_ship_bundle.py --json-in <temp> --public-key <pem>
'           --out <enc> --no-upload' (crypto auditado en Python, NO en VBA)
'        5) Shred del temp JSON
'        6) Pregunta: subir por SSH? (Ruta 1). Si No, deja el .enc para subir por el
'           panel del navegador (Ruta 2).
'
'  Por que VBA -> Python y no VBA puro: RSA-OAEP-4096 + AES-256-GCM en VBA requiere
'  Win32 CryptoAPI (fragil, imposible de auditar). VBA lee sheets (su fortaleza) +
'  Shell a la lib 'cryptography' de Python (su fortaleza). Es exactamente el patron
'  de RunFullSyncCycle (VBA -> Shell -> PowerShell -> SSH).
'
'  Doctrina (RULE 00 + gates 6/10):
'  - paper_mode / DEPLOYER_* / MULTISIG / MAINNET_RPC NUNCA se serializan.
'  - Capital sigue en 0. Este modulo NO toca el executor, signer, ni paper_mode.
'  - El temp JSON se shred (overwrite + Kill). El .enc queda en DeployDir.
'
'  Requisitos: pythonw en PATH; openpyxl + cryptography instalados; la llave publica
'  RSA-4096 en <DeployDir>\arbx_bundle_public.pem (o la ruta del arg --public-key).

Private Const ENV_SHEET As String = ".env Production"
Private Const RPC_SHEET As String = "RPC Providers"
Private Const TOKENS_SHEET As String = "Tokens & Keys"
Private Const CHAIN_BUILDER_SHEET As String = "Chain Builder"
Private Const HDR_ROW As Long = 2
Private Const DATA_START_ROW As Long = 3

' Llaves que NUNCA se serializan (capa 1 de 3). Espejo el set NEVER_SHIP de Python.
Private Function IsNeverShip(ByVal key As String) As Boolean
    Select Case UCase$(Trim$(key))
        Case "ARBX_PAPER_MODE", "ARBX_PAPER_TRADE", "PAPER_MODE", _
             "DEPLOYER_PRIVATE_KEY", "DEPLOYER_KEY", "MULTISIG_ADDRESS", _
             "CONFIRM_MAINNET_DEPLOY", "MAINNET_RPC_URL"
            IsNeverShip = True
        Case Else
            IsNeverShip = False
    End Select
End Function

' Constantes universales chainlist.org (NO config de operador - gate 6 no aplica).
' Mainnets + testnets (Sepolia/Amoy reemplazan Mumbai/Holesky que estan deprecated).
' Public para que SyncRpcCatalog.bas lo reutilice (DRY - un solo mapa de chains).
Public Function ChainIdFor(ByVal chainName As String) As Variant
    Select Case Trim$(chainName)
        Case "Ethereum Mainnet":  ChainIdFor = 1
        Case "Optimism":          ChainIdFor = 10
        Case "BSC Mainnet":       ChainIdFor = 56
        Case "Gnosis":            ChainIdFor = 100
        Case "Polygon Mainnet":   ChainIdFor = 137
        Case "Base":              ChainIdFor = 8453
        Case "Arbitrum One":      ChainIdFor = 42161
        Case "Avalanche":         ChainIdFor = 43114
        Case "Linea":             ChainIdFor = 59144
        Case "Blast":             ChainIdFor = 81457
        Case "Scroll":            ChainIdFor = 534352
        ' Testnets (publias, sin key - utiles para shadow/sim contra fork)
        Case "Ethereum Sepolia":  ChainIdFor = 11155111
        Case "Ethereum Holesky":  ChainIdFor = 17000    ' DEPRECATED (EF shutdown 2025-09) - queda solo para ref
        Case "Polygon Amoy":      ChainIdFor = 80002    ' reemplaza Mumbai (deprecated)
        Case "Arbitrum Sepolia":  ChainIdFor = 421614
        Case "Optimism Sepolia":  ChainIdFor = 11155420
        Case "Base Sepolia":      ChainIdFor = 84532
        Case Else:                ChainIdFor = Empty
    End Select
End Function

' Meta canonica por chain_id: name, native_currency, explorer_url (chainlist.org).
Private Sub ChainMeta(ByVal cid As Long, ByRef nm As String, ByRef nat As String, ByRef expl As String)
    Select Case cid
        Case 1:       nm = "ethereum":          nat = "ETH":   expl = "https://etherscan.io"
        Case 10:      nm = "optimism":          nat = "ETH":   expl = "https://optimistic.etherscan.io"
        Case 56:      nm = "bsc":               nat = "BNB":   expl = "https://bscscan.com"
        Case 100:     nm = "gnosis":            nat = "xDAI":  expl = "https://gnosisscan.io"
        Case 137:     nm = "polygon":           nat = "MATIC": expl = "https://polygonscan.com"
        Case 8453:    nm = "base":              nat = "ETH":   expl = "https://basescan.org"
        Case 42161:   nm = "arbitrum":          nat = "ETH":   expl = "https://arbiscan.io"
        Case 43114:   nm = "avalanche":         nat = "AVAX":  expl = "https://snowtrace.io"
        Case 59144:   nm = "linea":             nat = "ETH":   expl = "https://lineascan.build"
        Case 81457:   nm = "blast":             nat = "ETH":   expl = "https://blastscan.io"
        Case 534352:  nm = "scroll":            nat = "ETH":   expl = "https://scrollscan.com"
        ' Testnets
        Case 11155111:nm = "sepolia":           nat = "ETH":   expl = "https://sepolia.etherscan.io"
        Case 17000:   nm = "holesky":           nat = "ETH":   expl = "https://holesky.etherscan.io"
        Case 80002:   nm = "polygon-amoy":      nat = "MATIC": expl = "https://amoy.polygonscan.com"
        Case 421614:  nm = "arbitrum-sepolia":  nat = "ETH":   expl = "https://sepolia.arbiscan.io"
        Case 11155420:nm = "optimism-sepolia":  nat = "ETH":   expl = "https://optimism-sepolia.blockscout.com"
        Case 84532:   nm = "base-sepolia":      nat = "ETH":   expl = "https://sepolia.basescan.org"
        Case Else:    nm = "": nat = "": expl = ""
    End Select
End Sub

' Factories canonica por chain_id (dex_name -> router address). Espejo gen_chain_env.py.
Private Function FactoriesJson(ByVal cid As Long) As String
    Dim f As String
    Select Case cid
        Case 1
            f = "{""dex_name"":""UniswapV2"",""address"":""0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f""}," & _
                "{""dex_name"":""UniswapV3"",""address"":""0x1F98431c8aD98523631AE4a59f267346ea31F984""}," & _
                "{""dex_name"":""SushiSwap"",""address"":""0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac""}"
        Case 10
            f = "{""dex_name"":""UniswapV3"",""address"":""0x1F98431c8aD98523631AE4a59f267346ea31F984""}," & _
                "{""dex_name"":""SushiSwap"",""address"":""0xc35DADB65012eC412f5fe79F3667b22B3A32B795""}"
        Case 56
            f = "{""dex_name"":""PancakeSwap V2"",""address"":""0x1097053Fd5911a4863cA7D0e6F3C73a8B2CDA8b9""}," & _
                "{""dex_name"":""PancakeSwap V3"",""address"":""0x0BFbCF9fa4f9C56B0F40a671Ad90E3DC94D20d4e""}," & _
                "{""dex_name"":""BiSwap"",""address"":""0x3a6d8cA21D1CF76F653A67577FA0FB271661792C""}"
        Case 137
            f = "{""dex_name"":""UniswapV3"",""address"":""0x1F98431c8aD98523631AE4a59f267346ea31F984""}," & _
                "{""dex_name"":""SushiSwap"",""address"":""0xc35DADB65012eC412f5fe79F3667b22B3A32B795""}"
        Case 8453
            f = "{""dex_name"":""UniswapV3"",""address"":""0x33128a8fC17869897dcEA68d25cD9Ec44D11BbfA""}"
        Case 42161
            f = "{""dex_name"":""UniswapV3"",""address"":""0x1F98431c8aD98523631AE4a59f267346ea31F984""}," & _
                "{""dex_name"":""SushiSwap"",""address"":""0xc35DADB65012eC412f5fe79F3667b22B3A32B795""}"
        Case Else
            FactoriesJson = "[]"
            Exit Function
    End Select
    FactoriesJson = "[" & f & "]"
End Function

Private Function DeployDir() As String
    DeployDir = ThisWorkbook.Path & "\arbx-env-deploy"
End Function

Private Function PublicKeyPath() As String
    ' Default alongside the workbook's deploy dir; operator may place it elsewhere.
    PublicKeyPath = DeployDir() & "\arbx_bundle_public.pem"
End Function

Private Function BundleEncPath() As String
    BundleEncPath = DeployDir() & "\arbx_config_bundle.json.enc"
End Function

Private Function TempJsonPath() As String
    TempJsonPath = DeployDir() & "\.arbx_bundle_tmp.json"
End Function

' ============================ JSON helpers ============================

Private Function JsonEscape(ByVal s As String) As String
    Dim t As String
    t = Replace(s, "\", "\\")
    t = Replace(t, """", "\""")
    t = Replace(t, vbCr, "\r")
    t = Replace(t, vbLf, "\n")
    t = Replace(t, vbTab, "\t")
    JsonEscape = t
End Function

Private Function J(ByVal s As String) As String
    J = """" & JsonEscape(CStr(s)) & """"
End Function

' ============================ sheet readers ============================

' Lee .env Production -> env_vars JSON (filtrando NEVER_SHIP) + contract_addresses.
' Devuelve env_vars JSON por retorno; contract_addresses via ByRef param.
Private Function BuildEnvVarsJson(ByRef contractAddrsJson As String) As String
    Dim ws As Worksheet, r As Long, lastRow As Long
    Dim k As String, v As String, n As Long, nc As Long
    Dim parts As Collection, cparts As Collection
    Set ws = ThisWorkbook.Worksheets(ENV_SHEET)
    lastRow = ws.Cells(ws.Rows.Count, 1).End(xlUp).Row
    Set parts = New Collection
    Set cparts = New Collection
    For r = DATA_START_ROW To lastRow
        k = Trim$(CStr(ws.Cells(r, 1).Value))
        v = CStr(ws.Cells(r, 2).Value)
        If Len(k) > 0 And Len(v) > 0 Then
            If IsNeverShip(k) Then GoTo nextRow
            ' Contract address keys -> contract_addresses object
            If IsContractKey(k) And IsValidAddress(v) Then
                cparts.Add J(k) & ":" & J(v)
            Else
                parts.Add J(k) & ":" & J(v)
            End If
        End If
nextRow:
    Next r
    BuildEnvVarsJson = "{" & JoinColl(parts, ",") & "}"
    contractAddrsJson = "{" & JoinColl(cparts, ",") & "}"
End Function

Private Function IsContractKey(ByVal k As String) As Boolean
    Select Case UCase$(Trim$(k))
        Case "ARBITRAGE_EXECUTOR", "FLASHLOAN_EXECUTOR", "ALLOWANCE_MANAGER", "ADMIN_TIMELOCK"
            IsContractKey = True
        Case Else
            IsContractKey = False
    End Select
End Function

Private Function IsValidAddress(ByVal v As String) As Boolean
    Dim s As String: s = Trim$(v)
    IsValidAddress = (Left$(s, 2) = "0x") And (Len(s) = 42)
End Function

' Lee RPC Providers -> Dictionary(chain_name -> Dictionary(proto -> Collection("prov=url"))).
Private Function LoadRpcCatalog() As Object
    Dim ws As Worksheet, r As Long, lastRow As Long
    Dim chain As String, proto As String, prov As String, url As String
    Dim catalog As Object, perChain As Object
    Set catalog = CreateObject("Scripting.Dictionary")
    Set ws = ThisWorkbook.Worksheets(RPC_SHEET)
    lastRow = ws.Cells(ws.Rows.Count, 1).End(xlUp).Row
    For r = 2 To lastRow
        chain = Trim$(CStr(ws.Cells(r, 1).Value))
        proto = UCase$(Trim$(CStr(ws.Cells(r, 2).Value)))
        prov = Trim$(CStr(ws.Cells(r, 3).Value))
        url = Trim$(CStr(ws.Cells(r, 4).Value))
        If Len(chain) = 0 Or Len(proto) = 0 Or Len(prov) = 0 Or Len(url) = 0 Then GoTo nxt
        If Not catalog.Exists(chain) Then Set catalog(chain) = CreateObject("Scripting.Dictionary")
        Set perChain = catalog(chain)
        If Not perChain.Exists(proto) Then Set perChain(proto) = CreateObject("Scripting.Dictionary")
        perChain(proto)(prov) = url
nxt:
    Next r
    Set LoadRpcCatalog = catalog
End Function

Private Function CsvProviders(ByVal protoDict As Object) As String
    Dim k As Variant, parts As Collection
    Set parts = New Collection
    For Each k In protoDict.Keys
        parts.Add CStr(k) & "=" & CStr(protoDict(k))
    Next k
    CsvProviders = JoinColl(parts, ",")
End Function

' Lee Chain Builder (col B = checkmark) -> array de chain_name activos.
Private Function ActiveChainNames() As Collection
    Dim ws As Worksheet, r As Long, lastRow As Long
    Dim chain As String, mark As String
    Set ActiveChainNames = New Collection
    On Error Resume Next
    Set ws = ThisWorkbook.Worksheets(CHAIN_BUILDER_SHEET)
    If ws Is Nothing Then Exit Function
    On Error GoTo 0
    lastRow = ws.Cells(ws.Rows.Count, 1).End(xlUp).Row
    For r = 4 To lastRow   ' Chain Builder data starts row 4 (mirror gen_chain_env.py)
        chain = Trim$(CStr(ws.Cells(r, 1).Value))
        mark = UCase$(Trim$(CStr(ws.Cells(r, 2).Value)))
        If Len(chain) > 0 And (mark = "V" Or mark = "X" Or mark = ChrW$(&H2713)) Then
            ActiveChainNames.Add chain
        End If
    Next r
End Function

' Build the chains array JSON from active chains + the RPC catalog.
Private Function BuildChainsJson(ByVal catalog As Object, ByVal active As Collection) As String
    Dim nm As String, nat As String, expl As String
    Dim cid As Variant, chain As Variant, perChain As Object
    Dim parts As Collection, rpcHttp As String, rpcWs As String
    Dim item As String
    Set parts = New Collection
    For Each chain In active
        cid = ChainIdFor(CStr(chain))
        If IsEmpty(cid) Then GoTo skipChain
        If Not catalog.Exists(CStr(chain)) Then GoTo skipChain
        Set perChain = catalog(CStr(chain))
        ChainMeta CLng(cid), nm, nat, expl
        rpcHttp = "": rpcWs = ""
        If perChain.Exists("HTTP") Then rpcHttp = CsvProviders(perChain("HTTP"))
        If perChain.Exists("WSS") Then rpcWs = CsvProviders(perChain("WSS"))
        If Len(rpcHttp) = 0 Then GoTo skipChain  ' chain needs at least HTTP RPCs
        item = "{" & _
            J("chain_id") & ":" & CStr(cid) & "," & _
            J("name") & ":" & J(nm) & "," & _
            J("native_currency") & ":" & J(nat) & "," & _
            J("explorer_url") & ":" & J(expl) & "," & _
            J("rpc_http") & ":" & J(rpcHttp) & "," & _
            J("rpc_ws") & ":" & J(rpcWs) & "," & _
            J("factories") & ":" & FactoriesJson(CLng(cid)) & "}"
        parts.Add item
skipChain:
    Next chain
    BuildChainsJson = "[" & JoinColl(parts, ",") & "]"
End Function

' Lee Tokens & Keys -> api_keys JSON (filtrando NEVER_SHIP).
Private Function BuildApiKeysJson() As String
    Dim ws As Worksheet, r As Long, lastRow As Long
    Dim k As String, v As String
    Dim parts As Collection
    Set parts = New Collection
    On Error Resume Next
    Set ws = ThisWorkbook.Worksheets(TOKENS_SHEET)
    If ws Is Nothing Then BuildApiKeysJson = "{}": Exit Function
    On Error GoTo 0
    lastRow = ws.Cells(ws.Rows.Count, 1).End(xlUp).Row
    For r = DATA_START_ROW To lastRow
        k = Trim$(CStr(ws.Cells(r, 1).Value))
        v = CStr(ws.Cells(r, 2).Value)
        If Len(k) > 0 And Len(v) > 0 And Not IsNeverShip(k) Then
            parts.Add J(k) & ":" & J(v)
        End If
    Next r
    BuildApiKeysJson = "{" & JoinColl(parts, ",") & "}"
End Function

Private Function JoinColl(ByVal c As Collection, ByVal sep As String) As String
    Dim i As Long, parts() As String, out As String
    If c.Count = 0 Then JoinColl = "": Exit Function
    ReDim parts(1 To c.Count)
    For i = 1 To c.Count
        parts(i) = c(i)
    Next i
    JoinColl = Join(parts, sep)
End Function

' ============================ orchestrator ============================

Public Sub ShipBundle()
    Dim bundleJson As String, contractAddrsJson As String, envVarsJson As String
    Dim chainsJson As String, apiKeysJson As String
    Dim catalog As Object, active As Collection
    Dim ff As Integer, tmpPath As String, encPath As String
    Dim sh As Object, cmd As String, rc As Long, out As String
    Dim r As VbMsgBoxResult
    On Error GoTo fail

    If Dir(DeployDir(), vbDirectory) = "" Then MkDir DeployDir()
    tmpPath = TempJsonPath()
    encPath = BundleEncPath()

    ' 1. Build the bundle JSON (mirror encrypt_and_ship_bundle.build_bundle).
    envVarsJson = BuildEnvVarsJson(contractAddrsJson)
    Set catalog = LoadRpcCatalog()
    Set active = ActiveChainNames()
    chainsJson = BuildChainsJson(catalog, active)
    apiKeysJson = BuildApiKeysJson()

    bundleJson = "{" & _
        J("schema_version") & ":" & J("1.0") & "," & _
        J("generated_at") & ":" & J(Format$(Now(), "yyyy-mm-ddThh:nn:ssZ")) & "," & _
        J("env_vars") & ":" & envVarsJson & "," & _
        J("chains") & ":" & chainsJson & "," & _
        J("api_keys") & ":" & apiKeysJson & "," & _
        J("contract_addresses") & ":" & contractAddrsJson & "}"

    ' 2. Write plaintext JSON to temp (shred after encrypt).
    ff = FreeFile
    Open tmpPath For Output As #ff
    Print #ff, bundleJson;
    Close #ff

    ' 3. Shell the Python encryptor (--json-in mode: Python only encrypts, reads no Excel).
    '    Wrap in 'cmd /c ... 2> log' so Python's stderr is captured (pythonw hides it
    '    by default - the 2> redirect surfaces tracebacks for diagnosis).
    If Dir(PublicKeyPath()) = "" Then
        ShredFile tmpPath
        MsgBox "Falta la llave publica RSA-4096:" & vbCrLf & PublicKeyPath() & vbCrLf & _
               "Colocala ahi (la descargas del VPS, es PUBLICA unicamente).", _
               vbCritical, "ArbX bundle"
        Exit Sub
    End If
    Dim stderrLog As String
    stderrLog = DeployDir() & "\.arbx_bundle_stderr.log"
    On Error Resume Next
    Kill stderrLog
    On Error GoTo fail
    cmd = "cmd /c chcp 65001 > nul & pythonw.exe """ & DeployDir() & "\encrypt_and_ship_bundle.py"" " & _
          "--json-in """ & tmpPath & """ " & _
          "--public-key """ & PublicKeyPath() & """ " & _
          "--out """ & encPath & """ --no-upload 2> """ & stderrLog & """"
    Set sh = CreateObject("WScript.Shell")
    rc = sh.Run(cmd, 0, True)   ' hidden, synchronous
    If rc <> 0 Or Dir(encPath) = "" Then
        Dim diag As String
        diag = ReadTail(stderrLog, 500)
        ShredFile tmpPath
        MsgBox "El encryptor Python fallo (exit " & rc & ")." & vbCrLf & vbCrLf & _
               IIf(Len(diag) > 0, "Python stderr:" & vbCrLf & diag & vbCrLf & vbCrLf, "") & _
               "Log completo: " & stderrLog, vbCritical, "ArbX bundle"
        Exit Sub
    End If

    ' 4. Shred the plaintext temp (layer 1 plaintext gone; only the .enc remains).
    ShredFile tmpPath

    ' 5. Ask: SSH upload now (Ruta 1)? If No, leave the .enc for browser upload (Ruta 2).
    r = MsgBox("Bundle encriptado OK:" & vbCrLf & encPath & vbCrLf & vbCrLf & _
               "Subir por SSH ahora al VPS (Ruta 1)?" & vbCrLf & _
               "[No = deja el .enc para subirlo por el panel del navegador (Ruta 2)]", _
               vbQuestion + vbYesNo + vbDefaultButton1, "ArbX bundle - upload")
    If r = vbYes Then
        cmd = "scp """ & encPath & """ arbx:/opt/arbitragex-v2/config/arbx_config_bundle.json.enc"
        rc = sh.Run(cmd, 0, True)
        If rc <> 0 Then
            MsgBox "scp fallo (exit " & rc & "). El .enc sigue en " & encPath & _
                   " - subelo por el panel del navegador.", vbExclamation, "ArbX bundle"
        Else
            MsgBox "Bundle subido al VPS (Ruta 1)." & vbCrLf & _
                   "Ahora dispara el importer desde el panel o via SSH.", _
                   vbInformation, "ArbX bundle"
        End If
    Else
        MsgBox "Listo para Ruta 2: abre el panel del navegador y sube:" & vbCrLf & encPath, _
               vbInformation, "ArbX bundle"
    End If
    Exit Sub
fail:
    MsgBox "ShipBundle fallo: " & Err.Description, vbCritical, "ArbX bundle"
End Sub

Private Sub ShredFile(ByVal path As String)
    Dim ff As Integer, sz As Long
    On Error Resume Next
    If Dir(path) = "" Then Exit Sub
    ff = FreeFile
    Open path For Binary Access Write As #ff
    sz = LOF(ff)
    If sz > 0 Then
        Dim z() As Byte
        ReDim z(0 To sz - 1)
        Put #ff, , z
    End If
    Close #ff
    Kill path
    On Error GoTo 0
End Sub

' Read the last <=maxChars of a (UTF-8) stderr log as binary, so VBA's ANSI
' auto-decode doesn't mangle the Python traceback. Returns "" if missing/empty.
Private Function ReadTail(ByVal path As String, ByVal maxChars As Long) As String
    Dim ff As Integer, bytes() As Byte, content As String
    On Error Resume Next
    If Dir(path) = "" Then ReadTail = "": Exit Function
    ff = FreeFile
    Open path For Binary Access Read As #ff
    If LOF(ff) > 0 Then
        ReDim bytes(0 To LOF(ff) - 1)
        Get #ff, , bytes
    End If
    Close #ff
    content = StrConv(bytes, vbUnicode)
    If Len(content) > maxChars Then content = Right$(content, maxChars)
    content = Replace(content, vbCrLf, vbLf)
    content = Replace(content, vbCr, vbLf)
    ReadTail = content
    On Error GoTo 0
End Function

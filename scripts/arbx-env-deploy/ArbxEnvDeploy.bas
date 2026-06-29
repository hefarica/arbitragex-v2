Attribute VB_Name = "ArbxEnvDeploy"
Option Explicit

' ArbitrageX .env sync macro (two-way) + master cascade.
'
'  RunFullSyncCycle  -> EL CICLO COMPLETO EN 1 CLICK (cascada secuencial):
'        1) PULL   corrobora las credenciales SOLICITADAS por el VPS (.env.example reconciliado
'                  con el .env) y las trae a la hoja ".env Production" con ESTADO por clave en col C
'                  (ok / FALTA-LLENAR / REQUERIDA / REEMPLAZAR).
'        2) review muestra cuántas faltan y pregunta si subir.
'        3) PUSH   escribe el .env del VPS con los valores del Excel + reinicia + verifica.
'
'  Tambien sueltas:
'    PULL  : PullEnvFromVps (VPS -> Excel)      |  Excel <- credenciales solicitadas + estado
'    PUSH  : DeployEnvDryRun / DeployEnvApply    |  Excel -> VPS (panel flotante)
'    ExportEnvFragment : solo exporta el fragmento local
'
'  Secretos viajan VPS <-> archivo local <-> Excel y el archivo temporal se borra; nada se imprime.
'  Transporte por el ssh propio del operador (sin credenciales aqui). paper_mode jamas se toca.

Private Const ENV_SHEET As String = ".env Production"
Private Const KEY_COL As Long = 1          ' A = variable name
Private Const VAL_COL As Long = 2          ' B = value
Private Const NOTE_COL As Long = 3         ' C = estado (ok / FALTA-LLENAR / REQUERIDA / REEMPLAZAR)
Private Const HDR_ROW As Long = 2
Private Const DATA_START_ROW As Long = 3
Private Const ENV_TABLE As String = "tbl_env_production"

Private Function DeployDir() As String
    DeployDir = ThisWorkbook.Path & "\arbx-env-deploy"
End Function

Private Function FragmentPath() As String
    FragmentPath = DeployDir() & "\arbx_env_fragment.env"
End Function

' ============================ PUSH: Excel -> VPS ============================

Private Function ExportCore() As Long
    Dim ws As Worksheet, r As Long, lastRow As Long
    Dim k As String, v As String, n As Long, ff As Integer
    Set ws = ThisWorkbook.Worksheets(ENV_SHEET)
    lastRow = ws.Cells(ws.Rows.Count, KEY_COL).End(xlUp).Row
    If Dir(DeployDir(), vbDirectory) = "" Then MkDir DeployDir()
    ff = FreeFile
    Open FragmentPath() For Output As #ff
    Print #ff, "# arbx_env_fragment.env - exported " & Format(Now, "yyyy-mm-dd hh:nn:ss")
    n = 0
    For r = DATA_START_ROW To lastRow
        k = Trim$(CStr(ws.Cells(r, KEY_COL).Value))
        v = CStr(ws.Cells(r, VAL_COL).Value)
        If Len(k) > 0 And Len(v) > 0 Then        ' brechas vacias NO se suben
            Print #ff, k & "=" & v
            n = n + 1
        End If
    Next r
    Close #ff
    ExportCore = n
End Function

Public Sub ExportEnvFragment()
    Dim n As Long
    On Error GoTo fail
    n = ExportCore()
    MsgBox "Exported " & n & " variables ->" & vbCrLf & FragmentPath(), vbInformation, "ArbX export"
    Exit Sub
fail:
    MsgBox "Export failed: " & Err.Description, vbCritical, "ArbX export"
End Sub

Private Sub RunDeploy(ByVal applyMode As Boolean)
    Dim gui As String, args As String, modeStr As String
    On Error GoTo fail
    gui = DeployDir() & "\arbx_deploy_gui.ps1"
    If Dir(gui) = "" Then
        MsgBox "Missing panel: " & gui, vbCritical, "ArbX deploy"
        Exit Sub
    End If
    ExportCore
    modeStr = IIf(applyMode, "apply", "dryrun")
    args = "-Sta -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File """ & gui & """ -Mode " & modeStr
    Shell "powershell.exe " & args, vbHide
    Exit Sub
fail:
    MsgBox "Deploy launch failed: " & Err.Description, vbCritical, "ArbX deploy"
End Sub

Public Sub DeployEnvDryRun()
    RunDeploy False
End Sub

Public Sub DeployEnvApply()
    Dim r As VbMsgBoxResult
    r = MsgBox("ADVERTENCIA: escribira credenciales en el .env de PRODUCCION (VPS) y " & _
               "REINICIARA searcher-rs + api-server." & vbCrLf & vbCrLf & _
               "Idempotente y respaldada (backup .bak_*), pero toca el sistema en vivo. " & _
               "paper_mode NO se modifica (capital sigue en 0)." & vbCrLf & vbCrLf & _
               "Continuar?", vbExclamation + vbYesNo + vbDefaultButton2, "ArbX deploy -> PRODUCCION")
    If r = vbYes Then RunDeploy True
End Sub

' ============================ PULL: VPS -> Excel ============================

' Descarga el manifiesto de credenciales SOLICITADAS (TSV KEY/VALUE/ESTADO) y lo upsert-ea
' en ".env Production". No UI -> testeable via Application.Run. Devuelve filas sincronizadas
' (-1 si el pull falla).
Public Function PullEnvCore() As Long
    Dim sh As Object, ps As String, pulled As String, rc As Long, n As Long, ff As Integer
    pulled = DeployDir() & "\.arbx_manifest.tsv"
    On Error Resume Next
    If Dir(pulled) <> "" Then Kill pulled
    On Error GoTo 0
    ps = "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File """ & _
         DeployDir() & "\arbx_pull_run.ps1"" -OutFile """ & pulled & """"
    Set sh = CreateObject("WScript.Shell")
    rc = sh.Run(ps, 0, True)              ' hidden, sincronico
    If rc <> 0 Or Dir(pulled) = "" Then
        PullEnvCore = -1
        Exit Function
    End If
    n = ImportEnvToSheet(pulled)
    EnsureEnvTable
    On Error Resume Next                  ' shred del temporal con secretos
    ff = FreeFile
    Open pulled For Output As #ff: Print #ff, "shredded": Close #ff
    Kill pulled
    On Error GoTo 0
    PullEnvCore = n
End Function

Public Sub PullEnvFromVps()
    Dim n As Long
    On Error GoTo fail
    n = PullEnvCore()
    If n < 0 Then
        MsgBox "Pull fallo: no se pudo traer el manifiesto del VPS (revisa ssh 'arbx').", vbCritical, "ArbX pull"
        Exit Sub
    End If
    MsgBox "Sincronizado: " & n & " credenciales solicitadas del VPS -> '.env Production'." & vbCrLf & _
           "Columna C = estado (ok / FALTA-LLENAR / REQUERIDA). Tabla dinamica '" & ENV_TABLE & "'.", _
           vbInformation, "ArbX pull"
    Exit Sub
fail:
    MsgBox "Pull failed: " & Err.Description, vbCritical, "ArbX pull"
End Sub

' Upsert TSV (KEY<TAB>VALUE<TAB>NOTES). El VALOR del VPS manda cuando viene; en una brecha
' (valor vacio) se PRESERVA lo que el operador ya escribio localmente. El estado va a col C.
Private Function ImportEnvToSheet(ByVal path As String) As Long
    Dim ws As Worksheet, ff As Integer, content As String, lines() As String, parts() As String
    Dim i As Long, line As String, k As String, v As String, note As String
    Dim lastRow As Long, r As Long, nextRow As Long, n As Long, tgt As Long
    Dim dict As Object
    Set ws = ThisWorkbook.Worksheets(ENV_SHEET)
    Set dict = CreateObject("Scripting.Dictionary")
    lastRow = ws.Cells(ws.Rows.Count, KEY_COL).End(xlUp).Row
    If lastRow < DATA_START_ROW Then lastRow = DATA_START_ROW - 1
    For r = DATA_START_ROW To lastRow
        k = Trim$(CStr(ws.Cells(r, KEY_COL).Value))
        If Len(k) > 0 Then If Not dict.Exists(k) Then dict.Add k, r
    Next r
    nextRow = lastRow + 1
    ff = FreeFile
    Open path For Input As #ff
    If LOF(ff) > 0 Then content = Input$(LOF(ff), ff)
    Close #ff
    content = Replace(content, vbCrLf, vbLf)
    content = Replace(content, vbCr, vbLf)
    lines = Split(content, vbLf)
    For i = LBound(lines) To UBound(lines)
        line = lines(i)
        If Len(Trim$(line)) = 0 Then GoTo cont
        parts = Split(line, vbTab)
        k = Trim$(parts(0))
        If Len(k) = 0 Then GoTo cont
        If UBound(parts) >= 1 Then v = parts(1) Else v = ""
        If UBound(parts) >= 2 Then note = parts(2) Else note = ""
        If dict.Exists(k) Then
            tgt = dict(k)
        Else
            tgt = nextRow
            ws.Cells(tgt, KEY_COL).Value = k
            dict.Add k, tgt
            nextRow = nextRow + 1
        End If
        If Len(v) > 0 Then ws.Cells(tgt, VAL_COL).Value = v   ' valor del VPS manda; brecha preserva fill local
        ws.Cells(tgt, NOTE_COL).Value = note
        n = n + 1
cont:
    Next i
    ImportEnvToSheet = n
End Function

Private Sub EnsureEnvTable()
    Dim ws As Worksheet, lo As ListObject, lastRow As Long, rng As Range
    On Error Resume Next
    Set ws = ThisWorkbook.Worksheets(ENV_SHEET)
    lastRow = ws.Cells(ws.Rows.Count, KEY_COL).End(xlUp).Row
    If lastRow < HDR_ROW Then Exit Sub
    Set rng = ws.Range(ws.Cells(HDR_ROW, 1), ws.Cells(lastRow, 3))
    Set lo = ws.ListObjects(ENV_TABLE)
    If lo Is Nothing Then
        Set lo = ws.ListObjects.Add(xlSrcRange, rng, , xlYes)
        lo.Name = ENV_TABLE
        lo.TableStyle = ""
    Else
        lo.Resize rng
    End If
    On Error GoTo 0
End Sub

' ===================== MASTER: ciclo completo en 1 click =====================
Public Sub RunFullSyncCycle()
    Dim n As Long, gaps As Long, r As VbMsgBoxResult
    Dim ws As Worksheet, rr As Long, lastRow As Long, st As String
    On Error GoTo fail
    ' 1) PULL: corroborar credenciales solicitadas del VPS -> Excel
    n = PullEnvCore()
    If n < 0 Then
        MsgBox "Ciclo abortado: el PULL del VPS fallo (revisa ssh 'arbx').", vbCritical, "ArbX ciclo"
        Exit Sub
    End If
    ThisWorkbook.Save
    ' 2) contar brechas (estado en col C)
    Set ws = ThisWorkbook.Worksheets(ENV_SHEET)
    lastRow = ws.Cells(ws.Rows.Count, KEY_COL).End(xlUp).Row
    gaps = 0
    For rr = DATA_START_ROW To lastRow
        st = UCase$(CStr(ws.Cells(rr, NOTE_COL).Value))
        If InStr(st, "FALTA") > 0 Or InStr(st, "REQUERIDA") > 0 Or InStr(st, "REEMPLAZAR") > 0 Then gaps = gaps + 1
    Next rr
    ' 3) review + PUSH
    r = MsgBox("PULL ok: " & n & " credenciales solicitadas sincronizadas (VPS -> Excel)." & vbCrLf & _
               gaps & " con brecha (FALTA-LLENAR / REQUERIDA / REEMPLAZAR) -> ver columna C." & vbCrLf & vbCrLf & _
               "Hacer PUSH ahora (Excel -> VPS: escribe .env + reinicia + verifica)?" & vbCrLf & _
               "[No = solo quedo el pull; llena las faltantes en '.env Production' y vuelve a correr el ciclo]", _
               vbQuestion + vbYesNo + vbDefaultButton2, "ArbX - Ciclo completo (1 click)")
    If r = vbYes Then RunDeploy True
    Exit Sub
fail:
    MsgBox "Ciclo fallo: " & Err.Description, vbCritical, "ArbX ciclo"
End Sub

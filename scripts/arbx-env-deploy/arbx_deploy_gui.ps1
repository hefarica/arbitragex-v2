param(
  [ValidateSet('apply','dryrun','safediff')][string]$Mode='safediff',
  [string]$VpsHost='arbx',
  [string]$EnvPath='/opt/arbitragex-v2/.env',
  [string]$DeployDir,
  [switch]$SelfTest
)
# Apple-style floating panel for the ArbitrageX .env deploy. No visible PowerShell
# console (launch with -WindowStyle Hidden). Runs arbx_fastdeploy.sh (1 ssh connection),
# shows status, auto-closes with a confirmation on success; on error stays open and
# shows the error log at the bottom. QuantumX dark theme.
$ErrorActionPreference='Stop'
if(-not $DeployDir){ $DeployDir = Split-Path -Parent $MyInvocation.MyCommand.Path }
Add-Type -AssemblyName PresentationFramework,PresentationCore,WindowsBase | Out-Null

$xaml = @'
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        WindowStyle="None" AllowsTransparency="True" Background="Transparent"
        ResizeMode="NoResize" Topmost="True" ShowInTaskbar="False"
        WindowStartupLocation="CenterScreen" Width="560" SizeToContent="Height">
  <Border CornerRadius="18" Background="#0B1226" BorderBrush="#222B3D" BorderThickness="1" Padding="22">
    <Border.Effect><DropShadowEffect BlurRadius="42" ShadowDepth="0" Opacity="0.55" Color="#000000"/></Border.Effect>
    <StackPanel>
      <StackPanel Orientation="Horizontal">
        <TextBlock Text="&#9670; ArbitrageX" Foreground="#427DFC" FontSize="18" FontWeight="Bold" FontFamily="Consolas"/>
        <TextBlock Text="  .env deploy" Foreground="#8E9CB1" FontSize="18" FontFamily="Consolas"/>
        <Border CornerRadius="8" Background="#1E3159" Padding="9,2" Margin="12,2,0,0" VerticalAlignment="Center">
          <TextBlock x:Name="ModeText" Text="MODE" Foreground="#F3F9FF" FontSize="11" FontFamily="Consolas"/>
        </Border>
      </StackPanel>
      <TextBlock x:Name="HostLine" Margin="0,5,0,0" Foreground="#8E9CB1" FontSize="11" FontFamily="Consolas"/>
      <TextBlock x:Name="StatusText" Margin="0,20,0,0" Foreground="#F3F9FF" FontSize="15" FontFamily="Consolas" Text="Iniciando..." TextWrapping="Wrap"/>
      <ProgressBar x:Name="Bar" IsIndeterminate="True" Height="4" Margin="0,14,0,0" Foreground="#427DFC" Background="#141C30" BorderThickness="0"/>
      <Border x:Name="LogBox" Margin="0,16,0,0" CornerRadius="10" Background="#040817" BorderBrush="#5A2530" BorderThickness="1" Visibility="Collapsed">
        <ScrollViewer VerticalScrollBarVisibility="Auto" MaxHeight="150" Padding="10">
          <TextBlock x:Name="LogText" Foreground="#E5879A" FontSize="11" FontFamily="Consolas" TextWrapping="Wrap"/>
        </ScrollViewer>
      </Border>
      <StackPanel Orientation="Horizontal" HorizontalAlignment="Right" Margin="0,16,0,0">
        <TextBlock x:Name="CountText" Foreground="#8E9CB1" FontSize="11" FontFamily="Consolas" VerticalAlignment="Center" Margin="0,0,12,0"/>
        <Button x:Name="CloseBtn" Content="Cerrar" Padding="18,7" Background="#1E3159" Foreground="#F3F9FF" BorderThickness="0" Cursor="Hand" Visibility="Collapsed" FontFamily="Consolas"/>
      </StackPanel>
    </StackPanel>
  </Border>
</Window>
'@

$win = [Windows.Markup.XamlReader]::Parse($xaml)
$ModeText=$win.FindName('ModeText'); $HostLine=$win.FindName('HostLine'); $StatusText=$win.FindName('StatusText')
$Bar=$win.FindName('Bar'); $LogBox=$win.FindName('LogBox'); $LogText=$win.FindName('LogText')
$CountText=$win.FindName('CountText'); $CloseBtn=$win.FindName('CloseBtn')

function B([string]$h){ New-Object Windows.Media.SolidColorBrush ([Windows.Media.Color][Windows.Media.ColorConverter]::ConvertFromString($h)) }

$ModeText.Text = $Mode.ToUpper()
$HostLine.Text = "$VpsHost  ->  $EnvPath"
$win.Add_MouseLeftButtonDown({ try{ $win.DragMove() }catch{} })
$CloseBtn.Add_Click({ $win.Close() })

$script:logfile = Join-Path $DeployDir ".arbx_last.log"
Remove-Item $script:logfile -Force -ErrorAction SilentlyContinue
$script:started = $null
function Read-Log { try { [IO.File]::ReadAllText($script:logfile) } catch { "" } }

function Show-Success([string]$summary){
  $Bar.Visibility='Collapsed'
  $StatusText.Foreground=(B '#2CBB67')
  $msg = if($Mode -eq 'apply'){"OK  Aplicado - servicios reiniciados - paper_mode intacto"}else{"OK  Dry-run - nada cambiado en el VPS"}
  if($summary){ $msg = $msg + "`n" + $summary }
  $StatusText.Text = $msg
  # auto-close countdown (script-scoped so the tick survives after this fn returns)
  $script:cd=4
  $CountText.Text = "cierra en $($script:cd)s"
  $script:ct = New-Object Windows.Threading.DispatcherTimer
  $script:ct.Interval=[TimeSpan]::FromSeconds(1)
  $script:ct.Add_Tick({ $script:cd--; if($script:cd -le 0){ $script:ct.Stop(); $win.Close() } else { $CountText.Text="cierra en $($script:cd)s" } })
  $script:ct.Start()
  $CloseBtn.Content="Cerrar ahora"; $CloseBtn.Visibility='Visible'
}
function Show-Error([string]$log){
  $Bar.Visibility='Collapsed'
  $StatusText.Foreground=(B '#E54056')
  $StatusText.Text = "X  Error en el deploy - revisa el log:"
  $LogText.Text = $log
  $LogBox.Visibility='Visible'
  $CloseBtn.Visibility='Visible'
  $CountText.Text=""
}

$timer = New-Object Windows.Threading.DispatcherTimer
$timer.Interval=[TimeSpan]::FromMilliseconds(300)
$timer.Add_Tick({
 try{
  if($SelfTest){ $timer.Stop(); Show-Success "self-test: panel render OK"; return }
  $txt = Read-Log
  $m = [regex]::Match($txt, '__ARBX_EXIT__=(\d+)')
  if($m.Success){
    $timer.Stop()
    $code=[int]$m.Groups[1].Value
    $body=($txt -replace '__ARBX_EXIT__=\d+\s*$','').Trim()
    if($code -eq 0){
      $sum=""
      foreach($ln in ($body -split "`n")){ if($ln -match 'totals:|ADD |CHANGE |SAME |upsert ok|updated in place|scanner|Started'){ $sum += ($ln.Trim()+"`n") } }
      Show-Success $sum.Trim()
    } else { Show-Error ("EXIT=$code`n$body") }
  } elseif($script:started -and ((Get-Date)-$script:started).TotalSeconds -gt 90){
    $timer.Stop(); Show-Error ("Timeout 90s sin respuesta del VPS.`n`n" + $txt)
  } else {
    $StatusText.Text = if($Mode -eq 'apply'){"Transfiriendo + aplicando en el VPS..."}else{"Transfiriendo + comparando (dry-run)..."}
  }
 } catch { $timer.Stop(); Show-Error ("handler error: " + $_.Exception.Message) }
})

$win.Add_Loaded({
  if($SelfTest){ $timer.Start(); return }
  try{
    # Launch a HIDDEN child PowerShell (NOT bash — MSYS bash can't run headless). The
    # child runs the pure-PS deploy (native OpenSSH) and writes a completion marker to
    # the log, which the timer polls. Robust + works fully hidden from Excel.
    $runner = Join-Path $DeployDir 'arbx_deploy_run.ps1'
    $logged = Join-Path $DeployDir 'arbx_deploy_logged.ps1'
    $script:started = Get-Date
    Start-Process -FilePath 'powershell.exe' -WindowStyle Hidden -ArgumentList @(
      '-NoProfile','-ExecutionPolicy','Bypass','-WindowStyle','Hidden','-File',$logged,
      '-Mode',$Mode,'-VpsHost',$VpsHost,'-EnvPath',$EnvPath,'-LogPath',$script:logfile,'-Runner',$runner) | Out-Null
    $timer.Start()
  } catch {
    Show-Error ("No se pudo lanzar el deploy: " + $_.Exception.Message)
  }
})

[void]$win.ShowDialog()
Remove-Item $script:logfile -Force -ErrorAction SilentlyContinue

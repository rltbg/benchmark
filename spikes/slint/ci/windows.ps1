# Windows x64 : build release puis exécution dans la session interactive du runner. usage: windows.ps1 <OUT>
param([string]$Out)
$ErrorActionPreference = "Continue"
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Out = (Resolve-Path $Out).Path
Set-Location "$env:GITHUB_WORKSPACE/spikes/slint"
& { rustc -Vv; cargo -V; (Get-CimInstance Win32_OperatingSystem).Caption } *> "$Out/versions.txt"
$sw = [Diagnostics.Stopwatch]::StartNew()
cmd /c "cargo build --release --locked 2>&1" | Tee-Object -FilePath "$Out/build.log"
$rc = $LASTEXITCODE
$sw.Stop(); $sec = [int]$sw.Elapsed.TotalSeconds
$exe = "target/release/slintspike.exe"
$status = "ok"; $notes = @()
if ($rc -ne 0 -or -not (Test-Path $exe)) { $status = "fail" }
$arts = @()
if ($status -eq "ok") {
  Copy-Item $exe "$Out/slintspike.exe"
  $arts = @("--artifact", "$Out/slintspike.exe")
  $env:SLINT_BACKEND = "winit-skia-software"; $env:SPIKE_AUTORUN = "1"; $env:SPIKE_KEEP = "1"
  $t0 = Get-Date
  $p = Start-Process -FilePath "$Out/slintspike.exe" -RedirectStandardOutput "$Out/run.log" -RedirectStandardError "$Out/run.err.log" -PassThru
  function Wait-Marker($pat, $timeout) {
    $end = (Get-Date).AddSeconds($timeout)
    while ((Get-Date) -lt $end) {
      if ((Test-Path "$Out/run.log") -and (Select-String -Path "$Out/run.log" -Pattern $pat -SimpleMatch -Quiet)) { return $true }
      Start-Sleep -Milliseconds 200
    }
    return $false
  }
  function Shot($name) {
    Add-Type -AssemblyName System.Windows.Forms, System.Drawing
    $b = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
    $bmp.Save("$Out/$name", [System.Drawing.Imaging.ImageFormat]::Png)
  }
  $metrics = @()
  if (Wait-Marker "SPIKE:ready" 90) { $metrics += "cold_start_ms_to_ready=$([int]((Get-Date) - $t0).TotalMilliseconds)" }
  if (Wait-Marker "SPIKE:tab index=1" 20) { Start-Sleep -Milliseconds 1500; Shot "shot-1-list.png" }
  if (Wait-Marker "SPIKE:tab index=2" 20) { Start-Sleep 2; Shot "shot-2-canvas.png"; $p.Refresh(); $metrics += "working_set_kb_canvas=$([int]($p.WorkingSet64/1024))" }
  if (Wait-Marker "SPIKE:done" 30) { Shot "shot-3-core.png"; $p.Refresh(); $metrics += "working_set_kb_end=$([int]($p.WorkingSet64/1024))" }
  Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
  if (-not (Select-String -Path "$Out/run.log" -Pattern "SPIKE:done" -SimpleMatch -Quiet)) { $status = "fail"; $notes += @("--note", "SPIKE:done absent") }
  if (-not (Select-String -Path "$Out/run.log" -Pattern "SPIKE:selftest ok" -SimpleMatch -Quiet)) { $status = "fail"; $notes += @("--note", "selftest non ok") }
  $notes += @("--note", "execution: session du runner windows-2025, SLINT_BACKEND=winit-skia-software, chiffres de CI")
  $notes += @("--note", ("metrics: " + ($metrics -join " ")))
}
python "$env:GITHUB_WORKSPACE/ci/record.py" --stack slint --platform windows --status $status --seconds $sec --out $Out @arts @notes
exit 0

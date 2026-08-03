# tools/dispatch.ps1 — QuotaPane floor-session dispatcher (M11d, stage 2).
#
# Watches prompts\queue\ for launcher files committed by the top tier and
# runs each as a headless Claude Code floor session in this repo. The human
# handover this replaces is only the courier step: specs, hard stops, owner
# publish/acceptance, and top-tier go-aheads are unchanged.
#
# Install (one-time), from the repo root in a PowerShell you leave open:
#   powershell -ExecutionPolicy Bypass -File tools\dispatch.ps1
# or register with Task Scheduler for login-time start.
#
# Permissions: the floor session's tool permissions come from the repo's
# checked-in Claude Code settings. NEVER add --dangerously-skip-permissions
# here — a headless floor with unbounded permissions has no one watching it.
param(
  [string]$Repo = "C:\dev\QuotaPane\QuotaPane",
  [int]$IntervalSec = 60
)
$queue = Join-Path $Repo "prompts\queue"
$done  = Join-Path $queue "done"
$logd  = Join-Path $Repo "reports\dispatch"
New-Item -ItemType Directory -Force -Path $queue, $done, $logd | Out-Null
Write-Host "dispatcher: watching $queue every ${IntervalSec}s (Ctrl+C stops; one session at a time)"
while ($true) {
  $next = Get-ChildItem $queue -Filter *.md -File -ErrorAction SilentlyContinue |
          Sort-Object Name | Select-Object -First 1
  if ($next) {
    $name  = [IO.Path]::GetFileNameWithoutExtension($next.Name)
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $log   = Join-Path $logd "$name.$stamp.log"
    Write-Host "dispatch: $($next.Name) -> $log"
    $launcher = Get-Content -Raw $next.FullName
    Push-Location $Repo
    try {
      & claude -p $launcher --output-format text 2>&1 | Tee-Object -FilePath $log
      $code = $LASTEXITCODE
    } finally { Pop-Location }
    Move-Item $next.FullName (Join-Path $done $next.Name) -Force
    Write-Host "dispatch: done $($next.Name) (exit $code); end-gate report expected in reports\"
  }
  Start-Sleep -Seconds $IntervalSec
}

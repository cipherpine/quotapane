# tools/dispatch.ps1 — QuotaPane floor-session dispatcher (M11d, stage 2).
#
# Watches prompts\queue\ for launcher files and runs each as a headless
# Claude Code floor session, with LIVE progress: the session emits one
# JSON event per turn (--output-format stream-json), which this script
# renders as compact console lines as they happen and appends raw to the
# log. The human handover this replaces is only the courier step: specs,
# hard stops, owner publish/acceptance, and top-tier go-aheads are
# unchanged.
#
# Install (one-time), from the repo root in a PowerShell you leave open:
#   powershell -ExecutionPolicy Bypass -File tools\dispatch.ps1
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

function Render-Event([string]$line, [string]$log) {
  Add-Content -Path $log -Value $line -Encoding utf8
  try { $o = $line | ConvertFrom-Json } catch { return }
  switch ($o.type) {
    "system" {
      if ($o.subtype -eq "init") { Write-Host ("  [start] model=" + $o.model) -ForegroundColor DarkGray }
    }
    "assistant" {
      foreach ($b in $o.message.content) {
        if ($b.type -eq "text" -and $b.text.Trim()) {
          $t = ($b.text -replace "\s+", " ").Trim()
          if ($t.Length -gt 160) { $t = $t.Substring(0, 160) + "…" }
          Write-Host ("  [floor] " + $t)
        }
        elseif ($b.type -eq "tool_use") {
          $arg = ""
          if ($b.input.command)   { $arg = $b.input.command }
          elseif ($b.input.file_path) { $arg = $b.input.file_path }
          elseif ($b.input.pattern)   { $arg = $b.input.pattern }
          $arg = ($arg -replace "\s+", " ")
          if ($arg.Length -gt 100) { $arg = $arg.Substring(0, 100) + "…" }
          Write-Host ("  [tool ] " + $b.name + "  " + $arg) -ForegroundColor DarkCyan
        }
      }
    }
    "result" {
      $secs = [math]::Round($o.duration_ms / 1000)
      Write-Host ("  [end  ] " + $o.subtype + " after ${secs}s, " + $o.num_turns + " turns") -ForegroundColor Green
    }
  }
}

while ($true) {
  $next = Get-ChildItem $queue -Filter *.md -File -ErrorAction SilentlyContinue |
          Sort-Object Name | Select-Object -First 1
  if ($next) {
    $name  = [IO.Path]::GetFileNameWithoutExtension($next.Name)
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $log   = Join-Path $logd "$name.$stamp.jsonl"
    Write-Host "dispatch: $($next.Name) -> $log"
    $launcher = Get-Content -Raw $next.FullName
    Push-Location $Repo
    try {
      & claude -p $launcher --output-format stream-json --verbose 2>&1 |
        ForEach-Object { Render-Event ([string]$_) $log }
      $code = $LASTEXITCODE
    } finally { Pop-Location }
    Move-Item $next.FullName (Join-Path $done $next.Name) -Force
    Write-Host "dispatch: done $($next.Name) (exit $code); reports land in reports\"
  }
  Start-Sleep -Seconds $IntervalSec
}

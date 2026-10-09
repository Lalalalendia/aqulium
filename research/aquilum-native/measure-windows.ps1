param(
  [Parameter(Mandatory = $true)][string]$IcedExe,
  [Parameter(Mandatory = $true)][string]$GpuiExe,
  [Parameter(Mandatory = $true)][string]$FixtureDir,
  [Parameter(Mandatory = $true)][string]$OutputDir
)

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
$fixtures = (Resolve-Path $FixtureDir).Path
$programs = @{
  iced = (Resolve-Path $IcedExe).Path
  gpui = (Resolve-Path $GpuiExe).Path
}
$cases = @(
  @{ Name = "one-1MiB"; Notes = @("note-01mb.md") },
  @{ Name = "one-5MiB"; Notes = @("note-05mb.md") },
  @{ Name = "one-20MiB"; Notes = @("note-20mb.md") },
  @{ Name = "three-5MiB"; Notes = @("note-05mb.md", "note-05mb-copy-2.md", "note-05mb-copy-3.md") }
)
$results = [System.Collections.Generic.List[object]]::new()
$incomplete = $false

foreach ($scenario in $cases) {
  $modes = @("iced", "gpui", "gpui", "iced")
  for ($i = 0; $i -lt $modes.Count; $i++) {
    $mode = $modes[$i]
    $arguments = @()
    foreach ($note in $scenario.Notes) {
      $file = Join-Path $fixtures $note
      if (!(Test-Path $file)) { throw "Missing fixture $file" }
      $arguments += @("--file", ('"' + $file + '"'))
    }
    $startedAt = [DateTime]::UtcNow
    $child = Start-Process -FilePath $programs[$mode] -ArgumentList $arguments -PassThru
    $windowMs = $null
    $samples = @()
    $exitCode = $null
    try {
      $timer = [Diagnostics.Stopwatch]::StartNew()
      for ($tries = 0; $tries -lt 140; $tries++) {
        Start-Sleep -Milliseconds 250
        $child.Refresh()
        if ($child.HasExited) { break }
        if ($child.MainWindowHandle -ne [IntPtr]::Zero) {
          $windowMs = $timer.Elapsed.TotalMilliseconds
          break
        }
      }
      # HWND creation is NOT "first paint" and NOT keyboard-to-pixel latency.
      if ($null -eq $windowMs) { $incomplete = $true; Write-Warning "Window not observed for $mode / $($scenario.Name) on this Windows runner" }
      Start-Sleep -Seconds 3
      for ($n = 0; $n -lt 6; $n++) {
        $child.Refresh()
        if ($child.HasExited) {
          $exitCode = $child.ExitCode
          break
        }
        $samples += [PSCustomObject]@{
          workingSetMiB = [math]::Round($child.WorkingSet64 / 1MB, 2)
          privateCommitMiB = [math]::Round($child.PrivateMemorySize64 / 1MB, 2)
          peakWorkingSetMiB = [math]::Round($child.PeakWorkingSet64 / 1MB, 2)
          threads = $child.Threads.Count
          windowPresent = ($child.MainWindowHandle -ne [IntPtr]::Zero)
        }
        Start-Sleep -Milliseconds 600
      }
      if ($samples.Count -lt 2) { $incomplete = $true }
    } finally {
      try {
        $child.Refresh()
        if (!$child.HasExited) {
          Stop-Process -Id $child.Id -Force -ErrorAction SilentlyContinue
          $child.WaitForExit(5000) | Out-Null
        }
      } catch { Write-Warning "Could not terminate app $mode PID $($child.Id): $_" }
      $child.Dispose()
    }
    $result = [PSCustomObject]@{
      scenario = $scenario.Name
      app = $mode
      repetition = $i
      openedFiles = $scenario.Notes.Count
      elapsedUntilWindowMs = $windowMs
      exitCode = $exitCode
      sampleCount = $samples.Count
      processMemory = $samples
      startedAtUtc = $startedAt.ToString("o")
      benchmarkWarning = "Iced text_editor vs GPUI Component Input multiline. Real keyboard/IME feature equivalence still unproven."
    }
    $results.Add($result)
    Write-Host ("AQUILUM_NATIVE_RAM " + ($result | ConvertTo-Json -Depth 8 -Compress))
  }
}
$path = Join-Path $OutputDir "raw-process-ram.json"
@($results) | ConvertTo-Json -Depth 10 | Set-Content -Path $path -Encoding UTF8
if ($incomplete) {
  Write-Error "Some native windows did not open or had too few samples. No winner may be declared."
  exit 1
}
Write-Host "AQUILUM_NATIVE_RAM_COMPLETE samples=$($results.Count)"

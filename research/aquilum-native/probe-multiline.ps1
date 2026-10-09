param(
  [Parameter(Mandatory=$true)][string]$IcedExe,
  [Parameter(Mandatory=$true)][string]$GpuiExe,
  [Parameter(Mandatory=$true)][string]$FixtureDir,
  [Parameter(Mandatory=$true)][string]$OutputDir
)
$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
$programs = @{ iced=(Resolve-Path $IcedExe).Path; gpui=(Resolve-Path $GpuiExe).Path }
$fixtures = (Resolve-Path $FixtureDir).Path
$cases = @(
  @{Name="one-1MiB"; Files=@("note-01mb.md")},
  @{Name="one-5MiB"; Files=@("note-05mb.md")},
  @{Name="one-20MiB"; Files=@("note-20mb.md")},
  @{Name="three-5MiB"; Files=@("note-05mb.md","note-05mb-copy-2.md","note-05mb-copy-3.md")}
)
$all = [System.Collections.Generic.List[object]]::new()
$anyNoWindow = $false
$budgetMiB = 2304
$maxWaitForWindowSeconds = 18

foreach ($case in $cases) {
  # ABBA ordering to reduce process startup noise on the same runner.
  $order = @("iced","gpui","gpui","iced")
  for ($index=0; $index -lt $order.Count; $index++) {
    $mode = $order[$index]
    $name = "$($case.Name)-$mode-$index"
    $stdout = Join-Path $OutputDir "$name-stdout.txt"
    $stderr = Join-Path $OutputDir "$name-stderr.txt"
    $args = @()
    foreach ($file in $case.Files) {
      $path = Join-Path $fixtures $file
      if (!(Test-Path $path)) { throw "Fixture not found: $path" }
      $args += "--file"
      $args += '"' + $path + '"'
    }
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $proc = Start-Process -FilePath $programs[$mode] -ArgumentList $args -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $windowMs = $null
    $startupExit = $null
    $measurements = [System.Collections.Generic.List[object]]::new()
    $budgetExceeded = $false
    try {
      for ($i=0; $i -lt ($maxWaitForWindowSeconds * 4); $i++) {
        Start-Sleep -Milliseconds 250
        $proc.Refresh()
        if ($proc.HasExited) {
          $startupExit = $proc.ExitCode
          break
        }
        if ($proc.MainWindowHandle -ne [IntPtr]::Zero) {
          $windowMs = [math]::Round($timer.Elapsed.TotalMilliseconds,2)
          break
        }
      }
      if ($null -eq $windowMs) {
        $anyNoWindow = $true
        # A process without a GUI is not a valid GUI memory sample.
        Write-Warning "NO_WINDOW case=$name; exit=$startupExit; HWND unavailable after $maxWaitForWindowSeconds s"
      } else {
        # Three seconds warmup then 10s of periodic samples, with private RAM safety cap.
        for ($n=0; $n -lt 26; $n++) {
          Start-Sleep -Milliseconds 500
          $proc.Refresh()
          if ($proc.HasExited) { break }
          $private = [math]::Round($proc.PrivateMemorySize64/1MB,2)
          $working = [math]::Round($proc.WorkingSet64/1MB,2)
          $measurements.Add([PSCustomObject]@{
            elapsedMs=[math]::Round($timer.Elapsed.TotalMilliseconds,2)
            privateMiB=$private
            workingSetMiB=$working
            mainWindowAlive=($proc.MainWindowHandle -ne [IntPtr]::Zero)
            threads=$proc.Threads.Count
          })
          if ($private -ge $budgetMiB) {
            $budgetExceeded=$true
            Write-Warning "RAM_LIMIT case=$name privateMiB=$private > threshold $budgetMiB"
            break
          }
        }
      }
    } finally {
      try {
        $proc.Refresh()
        if (!$proc.HasExited) {
          Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
          $proc.WaitForExit(4000) | Out-Null
        }
      } catch { Write-Warning "Unable to stop $name : $_" }
      $proc.Dispose()
    }
    $sampleCount=$measurements.Count
    $last=if($sampleCount){$measurements[$sampleCount-1]}else{$null}
    $result=[PSCustomObject]@{
      name=$name; app=$mode; workload=$case.Name; openedFiles=$case.Files.Count
      windowObserved=($null -ne $windowMs); hwndCreatedMs=$windowMs
      startupExitCode=$startupExit; sampleCount=$sampleCount
      ramSafetyLimitExceeded=$budgetExceeded
      finalSample=$last; samples=$measurements.ToArray()
      stdoutPath=$stdout; stderrPath=$stderr
      limitations="HWND not first paint; Iced and GPUI text editors have different feature sets; no input/IME correctness measured"
    }
    $all.Add($result)
    Write-Host ("AQUILUM_NATIVE_MULTILINE " + ($result | ConvertTo-Json -Depth 7 -Compress))
    if (!$result.windowObserved -and $mode -eq "gpui") {
      Write-Host "GPUI_STDOUT:"
      if (Test-Path $stdout) { Get-Content $stdout -Tail 35 }
      Write-Host "GPUI_STDERR:"
      if (Test-Path $stderr) { Get-Content $stderr -Tail 60 }
      break
    }
  }
  if ($anyNoWindow) { break }
}
$output=Join-Path $OutputDir "results.json"
@($all) | ConvertTo-Json -Depth 12 | Out-File -FilePath $output -Encoding utf8
if ($anyNoWindow) { throw "At least one actual GUI window failed to start: no valid GPUI-vs-Iced RAM comparison" }
Write-Host "AQUILUM_NATIVE_MULTILINE_SUCCESS runs=$($all.Count)"

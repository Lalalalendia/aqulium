param(
    [Parameter(Mandatory=$true)][string]$ExePath,
    [Parameter(Mandatory=$true)][string]$OutputDir,
    [int]$MaxSeconds = 230
)
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
$OutputDir = [System.IO.Path]::GetFullPath($OutputDir)
$env:AQUILUM_RAM_OUT = $OutputDir
$stagePath = Join-Path $OutputDir "current-stage.txt"
$eventsPath = Join-Path $OutputDir "events.jsonl"
$samplesPath = Join-Path $OutputDir "process-samples.csv"
$serverOut = Join-Path $OutputDir "telemetry-stdout.log"
$serverErr = Join-Path $OutputDir "telemetry-stderr.log"
$serverScript = Join-Path $PSScriptRoot "telemetry-server.mjs"
$nodeExe = (Get-Command node -ErrorAction Stop).Source
$server = $null
$app = $null
$rows = [System.Collections.Generic.List[object]]::new()
$finished = $false
try {
    $server = Start-Process -FilePath $nodeExe -ArgumentList @($serverScript) -PassThru -RedirectStandardOutput $serverOut -RedirectStandardError $serverErr
    $ready = $false
    for ($r = 0; $r -lt 40; $r++) {
        try {
            $result = Invoke-WebRequest "http://127.0.0.1:18713/health" -TimeoutSec 2
            if ($result.StatusCode -eq 200) { $ready = $true; break }
        } catch {}
        Start-Sleep -Milliseconds 250
    }
    if (!$ready) { throw "Telemetry HTTP listener did not start" }
    $app = Start-Process -FilePath $ExePath -WorkingDirectory (Split-Path -Parent $ExePath) -PassThru
    Write-Host "AQUILUM_PROCESS pid=$($app.Id) exe=$ExePath"

    $stopAt = (Get-Date).AddSeconds($MaxSeconds)
    $iteration = 0
    do {
        $app.Refresh()
        $stage = if (Test-Path $stagePath) { (Get-Content $stagePath -Raw).Trim() } else { "unknown" }
        $tree = @(Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId)
        $ids = [System.Collections.Generic.HashSet[int]]::new()
        [void]$ids.Add([int]$app.Id)
        $changed = $true
        while ($changed) {
            $changed = $false
            foreach ($process in $tree) {
                $parentPid = [int]$process.ParentProcessId
                $childPid = [int]$process.ProcessId
                if ($ids.Contains($parentPid) -and $ids.Add($childPid)) { $changed = $true }
            }
        }
        $appWorkingSet = 0.0; $webWorkingSet = 0.0; $othersWorkingSet = 0.0
        $appPrivate = 0.0; $webPrivate = 0.0; $othersPrivate = 0.0
        $webCount = 0; $alive = 0
        foreach ($pidValue in $ids) {
            try {
                $p = Get-Process -Id $pidValue -ErrorAction Stop
                $resident = [double]$p.WorkingSet64 / 1MB
                $commit = [double]$p.PrivateMemorySize64 / 1MB
                $alive += 1
                if ($pidValue -eq $app.Id) {
                    $appWorkingSet = $resident; $appPrivate = $commit
                } elseif ($p.ProcessName -match '^(msedgewebview2|msedge)$') {
                    $webWorkingSet += $resident; $webPrivate += $commit; $webCount += 1
                } else {
                    $othersWorkingSet += $resident; $othersPrivate += $commit
                }
            } catch {}
        }
        $rows.Add([PSCustomObject]@{
            Sample = $iteration
            UTC = [DateTime]::UtcNow.ToString("o")
            Stage = $stage
            ProcessCount = $alive
            WebViewCount = $webCount
            AppWorkingSetMiB = [math]::Round($appWorkingSet, 2)
            WebViewWorkingSetMiB = [math]::Round($webWorkingSet, 2)
            OtherWorkingSetMiB = [math]::Round($othersWorkingSet, 2)
            TotalWorkingSetMiB = [math]::Round(($appWorkingSet + $webWorkingSet + $othersWorkingSet), 2)
            AppPrivateCommitMiB = [math]::Round($appPrivate, 2)
            WebViewPrivateCommitMiB = [math]::Round($webPrivate, 2)
            TotalPrivateCommitMiB = [math]::Round(($appPrivate + $webPrivate + $othersPrivate), 2)
        })
        $iteration += 1
        if ($stage -eq "finished" -or $stage -eq "error") { $finished = $true; break }
        if ($app.HasExited) { Write-Host "AQUILUM_PROCESS_EXIT code=$($app.ExitCode)"; break }
        Start-Sleep -Milliseconds 850
    } while ((Get-Date) -lt $stopAt)
}
finally {
    if ($rows.Count -gt 0) {
        $rows | Export-Csv -Path $samplesPath -NoTypeInformation -Encoding utf8
        foreach ($group in ($rows | Group-Object Stage)) {
            $samples = @($group.Group)
            $ws = $samples | Measure-Object -Property TotalWorkingSetMiB -Average -Maximum
            $pr = $samples | Measure-Object -Property TotalPrivateCommitMiB -Average -Maximum
            $web = $samples | Measure-Object -Property WebViewWorkingSetMiB -Average
            Write-Host ("AQUILUM_RAM_SUMMARY stage={0} n={1} avg_ws_mib={2:N1} max_ws_mib={3:N1} avg_private_mib={4:N1} avg_webview_ws_mib={5:N1}" -f $group.Name, $samples.Count, $ws.Average, $ws.Maximum, $pr.Average, $web.Average)
        }
    }
    if ($app -ne $null) {
        try { if (!$app.HasExited) { & taskkill.exe /PID $app.Id /T /F | Out-Null } } catch {}
    }
    if ($server -ne $null) { try { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue } catch {} }
}
if (!(Test-Path $eventsPath)) { throw "No renderer events, check whether WebView2 launched" }
$events = @(Get-Content $eventsPath | Where-Object { $_.Trim() } | ForEach-Object { $_ | ConvertFrom-Json })
$stages = @($events | ForEach-Object { $_.stage })
if ($stages -contains "error") {
    $failure = ($events | Where-Object { $_.stage -eq "error" } | Select-Object -Last 1).details.message
    throw "Renderer failed: $failure"
}
if (!$finished -or $stages -notcontains "one_tab_stable" -or $stages -notcontains "three_tabs_stable" -or $stages -notcontains "switched_first_stable" -or $stages -notcontains "finished") {
    throw "Incomplete real WebView scenario: stages=$($stages -join ',')"
}
$viewSamples = @($rows | Where-Object { $_.Stage -in @("one_tab_stable","two_tabs_stable","three_tabs_stable","switched_first_stable") })
if ($viewSamples.Count -eq 0 -or @($viewSamples | Where-Object { $_.WebViewCount -gt 0 }).Count -eq 0) {
    throw "No WebView2 child processes in the real editor phase"
}
Write-Host "AQUILUM_RAM_VALIDATED complete WebView2 editor scenario"

param(
  [Parameter(Mandatory=$true)][string]$FullInputExe,
  [Parameter(Mandatory=$true)][string]$RopeExe,
  [Parameter(Mandatory=$true)][string]$FixtureDir,
  [Parameter(Mandatory=$true)][string]$OutputDir
)
$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
$folder=(Resolve-Path $FixtureDir).Path
$save=(Resolve-Path $OutputDir).Path
$apps=@{ full=(Resolve-Path $FullInputExe).Path; rope=(Resolve-Path $RopeExe).Path }
$cases=@(
  @{id="one-1MiB";files=@("note-01mb.md")},
  @{id="one-5MiB";files=@("note-05mb.md")},
  @{id="one-20MiB";files=@("note-20mb.md")},
  @{id="three-5MiB";files=@("note-05mb.md","note-05mb-copy-2.md","note-05mb-copy-3.md")}
)
$results=[System.Collections.Generic.List[object]]::new()
$capMiB=2304

foreach($case in $cases) {
  $order=@("full","rope","rope","full")
  for($index=0;$index -lt $order.Length;$index++) {
    $mode=$order[$index]
    $name="$($case.id)-$mode-$index"
    $exe=$apps[$mode]
    $args=@()
    foreach($file in $case.files) {
      $filePath=Join-Path $folder $file
      if(!(Test-Path $filePath)){throw "Missing input: $filePath"}
      $args+=@("--file", ('"'+$filePath+'"'))
    }
    $stdout=Join-Path $save "$name-out.txt"
    $stderr=Join-Path $save "$name-err.txt"
    $timer=[Diagnostics.Stopwatch]::StartNew()
    $proc=Start-Process -FilePath $exe -ArgumentList $args -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $windowMs=$null
    $overLimit=$false
    $samples=[System.Collections.Generic.List[object]]::new()
    try{
      for($step=0;$step -lt 100;$step++){
        Start-Sleep -Milliseconds 250
        $proc.Refresh()
        if($proc.HasExited){break}
        if($proc.MainWindowHandle -ne [IntPtr]::Zero){
          $windowMs=[math]::Round($timer.Elapsed.TotalMilliseconds,2)
          break
        }
      }
      if($null -ne $windowMs) {
        # Collect a real time series, not one startup sample.
        for($k=0;$k -lt 22;$k++){
          Start-Sleep -Milliseconds 600
          $proc.Refresh()
          if($proc.HasExited){break}
          $private=[math]::Round($proc.PrivateMemorySize64/1MB,2)
          $samples.Add([PSCustomObject]@{
            atMs=[math]::Round($timer.Elapsed.TotalMilliseconds,2)
            privateMiB=$private
            workingSetMiB=[math]::Round($proc.WorkingSet64/1MB,2)
            processAlive=(!$proc.HasExited)
            hwndPresent=($proc.MainWindowHandle -ne [IntPtr]::Zero)
          })
          if($private -ge $capMiB) {
            $overLimit=$true
            Write-Warning "BENCH_CAP name=$name privateMiB=$private"
            break
          }
        }
      }
    }finally{
      try{
        $proc.Refresh()
        if(!$proc.HasExited){
          Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
          $proc.WaitForExit(4000)|Out-Null
        }
      }catch{Write-Warning "Could not stop $name : $_"}
      $proc.Dispose()
    }
    $last=if($samples.Count){$samples[$samples.Count-1]}else{$null}
    $result=[PSCustomObject]@{
      scenario=$case.id; variant=$mode; iteration=$index
      files=$case.files.Count; windowObserved=($null -ne $windowMs)
      hwndCreatedMs=$windowMs; sampleCount=$samples.Count
      stableSamples=($samples.Count -ge 14)
      finalSample=$last; safetyLimitExceeded=$overLimit
      maxPrivateMiB=($samples|Measure-Object -Property privateMiB -Maximum).Maximum
      samples=$samples.ToArray()
      stderrPath=$stderr
    }
    $results.Add($result)
    Write-Host ("AQUILUM_GPUI_ROPE_GUI_RAM "+($result|ConvertTo-Json -Depth 8 -Compress))
    if($null -eq $windowMs -or ($samples.Count -lt 14 -and !$overLimit)) {
      if(Test-Path $stderr){Write-Host "ERRORS "+(Get-Content $stderr -Raw)}
      @($results)|ConvertTo-Json -Depth 9|Out-File -FilePath (Join-Path $save "results.json") -Encoding utf8
      throw "Invalid Windows GUI experiment, name=$name hwnd=$windowMs samples=$($samples.Count)"
    }
    if($mode -eq "rope" -and $overLimit) {
      @($results)|ConvertTo-Json -Depth 9|Out-File -FilePath (Join-Path $save "results.json") -Encoding utf8
      throw "Bounded GPUI exceeded safe RAM budget $capMiB MiB on $name"
    }
  }
}
@($results)|ConvertTo-Json -Depth 9|Out-File -FilePath (Join-Path $save "results.json") -Encoding utf8
$summary=foreach($case in $cases){
  foreach($mode in @("full","rope")){
    $rows=@($results|Where-Object{$_.scenario -eq $case.id -and $_.variant -eq $mode})
    [PSCustomObject]@{
      scenario=$case.id;variant=$mode
      runs=$rows.Count
      meanFinalPrivateMiB=[math]::Round(($rows | ForEach-Object {$_.finalSample.privateMiB}|Measure-Object -Average).Average,2)
      maxObservedPrivateMiB=[math]::Round(($rows | Measure-Object -Property maxPrivateMiB -Maximum).Maximum,2)
      reachedCap=(@($rows|Where-Object{$_.safetyLimitExceeded}).Count)
    }
  }
}
@($summary)|Export-Csv -Path (Join-Path $save "summary.csv") -NoTypeInformation -Encoding utf8
$summary|ForEach-Object{Write-Host ("AQUILUM_GPUI_ROPE_GUI_SUMMARY "+($_|ConvertTo-Json -Compress))}
Write-Host "AQUILUM_GPUI_ROPE_GUI_FINISHED runs=$($results.Count)"

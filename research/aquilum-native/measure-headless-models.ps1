param(
  [Parameter(Mandatory=$true)][string]$RopeExe,
  [Parameter(Mandatory=$true)][string]$IcedModelExe,
  [Parameter(Mandatory=$true)][string]$FixtureDir,
  [Parameter(Mandatory=$true)][string]$OutputDir
)
$ErrorActionPreference="Stop"
$rope=(Resolve-Path $RopeExe).Path
$iced=(Resolve-Path $IcedModelExe).Path
$fixtures=(Resolve-Path $FixtureDir).Path
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
$directory=(Resolve-Path $OutputDir).Path
$results=[System.Collections.Generic.List[object]]::new()
$capMiB=2304

foreach($mb in @(1,5,20)) {
  $file=(Resolve-Path (Join-Path $fixtures ("note-{0:D2}mb.md" -f $mb))).Path
  $expectedBytes=(Get-Item $file).Length
  $programs=@("iced","rope","rope","iced")
  for($i=0;$i -lt $programs.Count;$i++){
    $kind=$programs[$i]
    $tag="model-$mb-$kind-$i"
    $out=Join-Path $directory "$tag-out.txt"
    $err=Join-Path $directory "$tag-err.txt"
    $executable=if($kind -eq "rope"){$rope}else{$iced}
    $proc=Start-Process -FilePath $executable -ArgumentList @('"' + $file + '"') -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
    $ready=$false
    $limit=$false
    $samples=[System.Collections.Generic.List[object]]::new()
    $timer=[Diagnostics.Stopwatch]::StartNew()
    try {
      for($step=0;$step -lt 150;$step++){
        Start-Sleep -Milliseconds 260
        $proc.Refresh()
        if($proc.HasExited){break}
        $private=[math]::Round($proc.PrivateMemorySize64/1MB,2)
        $working=[math]::Round($proc.WorkingSet64/1MB,2)
        $samples.Add([PSCustomObject]@{
          elapsedMs=[math]::Round($timer.Elapsed.TotalMilliseconds,2)
          privateMiB=$private; workingSetMiB=$working
        })
        if($private -ge $capMiB){$limit=$true;break}
        if(Test-Path $out){
          $needle=if($kind -eq "rope"){"AQUILUM_ROPE_MODEL_READY"}else{"AQUILUM_ICED_MODEL_READY"}
          if(Select-String -Path $out -Pattern $needle -Quiet){$ready=$true;break}
        }
      }
      if($ready){
        Start-Sleep -Seconds 1
        $proc.Refresh()
        if(!$proc.HasExited){
          $samples.Add([PSCustomObject]@{
            elapsedMs=[math]::Round($timer.Elapsed.TotalMilliseconds,2)
            privateMiB=[math]::Round($proc.PrivateMemorySize64/1MB,2)
            workingSetMiB=[math]::Round($proc.WorkingSet64/1MB,2)
          })
        }
      }
    }finally{
      try{
        $proc.Refresh()
        if(!$proc.HasExited) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
      }catch{}
      $proc.Dispose()
    }
    $readout=if(Test-Path $out){Get-Content $out -Raw}else{""}
    $errors=if(Test-Path $err){Get-Content $err -Raw}else{""}
    $peakPrivate=($samples | Measure-Object -Property privateMiB -Maximum).Maximum
    $peakWorking=($samples | Measure-Object -Property workingSetMiB -Maximum).Maximum
    $parsed=[regex]::Match($readout,'AQUILUM_(?:ICED|ROPE)_MODEL_READY .*?build_ms=(?<ms>[0-9.]+)')
    if($kind -eq "rope" -and $ready) {
      $match=[regex]::Match($readout,'view_limit_chars=(?<chars>\d+) max_materialized_view_bytes=(?<max>\d+)')
      if(!$match.Success -or [int]$match.Groups["chars"].Value -gt 16384 -or [int]$match.Groups["max"].Value -gt 65536){
        throw "Rope viewport exceeded bounded view budget"
      }
      if($readout -notmatch "bytes=$expectedBytes "){throw "Rope text byte count mismatch"}
    }
    $result=[PSCustomObject]@{
      kind=$kind; workloadMiB=$mb; repeat=$i; inputBytes=$expectedBytes
      modelReady=$ready; overLimit=$limit; safetyCapMiB=$capMiB
      modelConstructionMs=if($parsed.Success){[double]$parsed.Groups["ms"].Value}else{$null}
      peakPrivateMiB=$peakPrivate; peakWorkingSetMiB=$peakWorking
      sampleCount=$samples.Count; samples=$samples.ToArray()
      outputFile=$out; errorFile=$err
    }
    $results.Add($result)
    Write-Host ("AQUILUM_NATIVE_MODEL_RAM "+($result|ConvertTo-Json -Depth 6 -Compress))
    if($kind -eq "rope" -and (!$ready -or $limit)) {
      Write-Host "ROPE_STDOUT: $readout"
      Write-Host "ROPE_STDERR: $errors"
      throw "Rope failed workload $mb MiB (ready=$ready overLimit=$limit)"
    }
    if($kind -eq "iced" -and !$ready -and !$limit){
      throw "Iced comparison did not initialize or hit expected safety limit, workload $mb MiB: $errors"
    }
  }
}
@($results)|ConvertTo-Json -Depth 10|Out-File -FilePath (Join-Path $directory "results.json") -Encoding utf8
$summary=foreach($mb in @(1,5,20)) {
  foreach($kind in @("iced","rope")) {
    $values=@($results|Where-Object {$_.workloadMiB -eq $mb -and $_.kind -eq $kind})
    [PSCustomObject]@{
      workloadMiB=$mb;model=$kind;runs=$values.Count
      ready=(@($values | Where-Object {$_.modelReady}).Count)
      safetyCapExceeded=(@($values|Where-Object{$_.overLimit}).Count)
      meanMaxPrivateMiB=[math]::Round(($values|Measure-Object -Property peakPrivateMiB -Average).Average,2)
      meanConstructMs=[math]::Round((@($values|Where-Object{$_.modelReady})|Measure-Object -Property modelConstructionMs -Average).Average,3)
    }
  }
}
@($summary)|Export-Csv -Path (Join-Path $directory "summary.csv") -NoTypeInformation -Encoding utf8
$summary|ForEach-Object{Write-Host ("AQUILUM_NATIVE_MODEL_SUMMARY "+($_|ConvertTo-Json -Compress))}
Write-Host "AQUILUM_NATIVE_MODEL_COMPARE_COMPLETE runs=$($results.Count)"

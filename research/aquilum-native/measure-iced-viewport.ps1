param(
  [Parameter(Mandatory=$true)][string]$ExePath,
  [Parameter(Mandatory=$true)][string]$FixtureDir,
  [Parameter(Mandatory=$true)][string]$OutputDir
)
$ErrorActionPreference="Stop"
New-Item -Path $OutputDir -ItemType Directory -Force | Out-Null
$executable=(Resolve-Path $ExePath).Path
$fixtures=(Resolve-Path $FixtureDir).Path
$cases=@(
  @{Name="one-1MiB";Notes=@("note-01mb.md")},
  @{Name="one-5MiB";Notes=@("note-05mb.md")},
  @{Name="one-20MiB";Notes=@("note-20mb.md")},
  @{Name="three-5MiB";Notes=@("note-05mb.md","note-05mb-copy-2.md","note-05mb-copy-3.md")}
)
$results=[System.Collections.Generic.List[object]]::new()
foreach($case in $cases){
  for($n=0;$n -lt 2;$n++){
    $name="$($case.Name)-$n"
    $args=@()
    foreach($file in $case.Notes){$args += "--file";$args += '"' + (Join-Path $fixtures $file) + '"'}
    $stdout=Join-Path $OutputDir "$name-stdout.log"
    $stderr=Join-Path $OutputDir "$name-stderr.log"
    $timer=[Diagnostics.Stopwatch]::StartNew()
    $p=Start-Process -FilePath $executable -ArgumentList $args -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $windowMs=$null
    $samples=[System.Collections.Generic.List[object]]::new()
    $capped=$false
    try {
      for($wait=0;$wait -lt 60;$wait++){
        Start-Sleep -Milliseconds 250
        $p.Refresh()
        if($p.HasExited){break}
        if($p.MainWindowHandle -ne [IntPtr]::Zero){
          $windowMs=[math]::Round($timer.Elapsed.TotalMilliseconds,2)
          break
        }
      }
      if($null -ne $windowMs){
        for($i=0;$i -lt 30;$i++){
          Start-Sleep -Milliseconds 500
          $p.Refresh()
          if($p.HasExited){break}
          $private=[math]::Round($p.PrivateMemorySize64/1MB,2)
          $samples.Add([PSCustomObject]@{
            elapsedMs=[math]::Round($timer.Elapsed.TotalMilliseconds,1)
            privateMiB=$private;wsMiB=[math]::Round($p.WorkingSet64/1MB,2)
            hwndPresent=($p.MainWindowHandle -ne [IntPtr]::Zero)
          })
          if($private -gt 2304){$capped=$true;break}
        }
      }
    } finally {
      try {
        $p.Refresh()
        if(!$p.HasExited){Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue}
      }catch{}
      $p.Dispose()
    }
    $last=if($samples.Count){$samples[$samples.Count-1]}else{$null}
    $row=[PSCustomObject]@{
      scenario=$case.Name;run=$n;files=$case.Notes.Count;
      hwndCreatedMs=$windowMs;sampleCount=$samples.Count;
      exceeded2p25GiB=$capped;final=$last;samples=$samples.ToArray()
      stderr=$stderr;stdout=$stdout
    }
    $results.Add($row)
    Write-Host ("AQUILUM_ICED_VIEWPORT_RAM " + ($row | ConvertTo-Json -Depth 7 -Compress))
    if($null -eq $windowMs -or $samples.Count -lt 20 -or $capped){
      Write-Host "STDERR:";if(Test-Path $stderr){Get-Content $stderr -Tail 40}
      $results | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $OutputDir "results.json") -Encoding UTF8
      throw "Iced bounded editor did not produce stable GUI samples: $name"
    }
  }
}
$results | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $OutputDir "results.json") -Encoding UTF8
"AQUILUM_ICED_VIEWPORT_RAM_VALIDATED runs=$($results.Count)"

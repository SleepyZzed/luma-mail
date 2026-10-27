param([Parameter(Mandatory=$true)][string]$Binary,[Parameter(Mandatory=$true)][string]$Evidence)
$ErrorActionPreference='Stop'
$binaryPath=(Resolve-Path -LiteralPath $Binary).Path
$evidencePath=[IO.Path]::GetFullPath((Join-Path (Get-Location) $Evidence))
New-Item -ItemType Directory -Force -Path (Join-Path $evidencePath 'config') | Out-Null
[IO.File]::WriteAllText((Join-Path $evidencePath 'config/settings.json'),'{"cache_mib":0,"accounts":[],"close_to_tray":false}')
$env:LUMA_TEST_ROOT=$evidencePath
$stdout=Join-Path $evidencePath 'performance.stdout';$stderr=Join-Path $evidencePath 'performance.stderr'
$process=Start-Process -FilePath $binaryPath -ArgumentList '--performance-diagnostics' -PassThru -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr
$null=$process.Handle # Preserve exit status in Windows PowerShell 5.1.
$timer=[Diagnostics.Stopwatch]::StartNew();$samples=@();$previous=@{};$last=0.0
try{
    while(-not $process.HasExited){
        Start-Sleep -Seconds 5
        if($process.HasExited){break}
        if($timer.Elapsed.TotalSeconds -gt 155){throw 'Windows performance fixture timed out'}
        $ids=[Collections.Generic.HashSet[int]]::new();[void]$ids.Add($process.Id)
        $all=@(Get-CimInstance Win32_Process -Property ProcessId,ParentProcessId)
        do{$before=$ids.Count;foreach($item in $all){if($ids.Contains([int]$item.ParentProcessId)){[void]$ids.Add([int]$item.ProcessId)}}}while($ids.Count -gt $before)
        $members=@(Get-Process -Id ([int[]]$ids) -ErrorAction SilentlyContinue)
        $counters=@(Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Property IDProcess,WorkingSetPrivate | Where-Object {$ids.Contains([int]$_.IDProcess)})
        $private=($counters | Measure-Object WorkingSetPrivate -Sum).Sum
        $commit=($members | Measure-Object PrivateMemorySize64 -Sum).Sum
        $working=($members | Measure-Object WorkingSet64 -Sum).Sum
        $now=$timer.Elapsed.TotalSeconds;$delta=0.0;$next=@{}
        foreach($member in $members){$value=$member.TotalProcessorTime.TotalSeconds;$next[$member.Id]=$value;if($previous.ContainsKey($member.Id)){$delta+=[Math]::Max(0,$value-$previous[$member.Id])}}
        $phase=Get-Content -LiteralPath $stdout | Where-Object {$_ -match '^PERF '} | Select-Object -Last 1
        $cpu=if($last -gt 0){[Math]::Round(100*$delta/($now-$last),3)}else{$null}
        $samples+=@{elapsed_seconds=[Math]::Round($now,2);phase=$phase;processes=$members.Count;private_working_set_bytes=[long]$private;private_commit_bytes=[long]$commit;working_set_sum_bytes=[long]$working;cpu_one_core_percent=$cpu;cpu_machine_percent=if($null -ne $cpu){[Math]::Round($cpu/[Environment]::ProcessorCount,4)}else{$null}}
        $previous=$next;$last=$now
    }
    $process.WaitForExit()
    if($process.ExitCode -ne 0){Get-Content -LiteralPath $stderr -TotalCount 25;throw 'Windows performance fixture failed'}
    if((Test-Path (Join-Path $evidencePath 'cache')) -and (Get-ChildItem (Join-Path $evidencePath 'cache') -File -Recurse | Select-Object -First 1)){throw 'Performance fixture persisted a zero-budget mail cache'}
    $report=@{binary_sha256=(Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash.ToLower();metric='Windows private working set and private commit; not Linux PSS';includes='Luma and recursively discovered child renderer/helper processes';cpu='Settled per-process CPU deltas, one-core and whole-machine percentages; short-lived exited helpers can be missed';samples=$samples}
    $report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidencePath 'windows-performance.json') -Encoding UTF8
    Get-Content -LiteralPath $stdout
    $samples | Group-Object phase | ForEach-Object {$group=$_.Group | Select-Object -Last 1;[pscustomobject]@{Phase=$_.Name;Processes=$group.processes;PrivateWorkingSetMiB=[Math]::Round($group.private_working_set_bytes/1MB,2);CPUOneCorePercent=$group.cpu_one_core_percent}} | Format-Table
}finally{
    if(-not $process.HasExited){Stop-Process -Id $process.Id}
    Remove-Item Env:LUMA_TEST_ROOT -ErrorAction SilentlyContinue
}

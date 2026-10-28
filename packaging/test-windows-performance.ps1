param([Parameter(Mandatory=$true)][string]$Binary,[Parameter(Mandatory=$true)][string]$Evidence,[switch]$Fluid)
$ErrorActionPreference='Stop'
$previousEnvironment=@{};foreach($name in @('LUMA_TEST_ROOT','LUMA_PREVIEW_HTML','LUMA_RESOURCE_REPORT')){$previousEnvironment[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
$binaryPath=(Resolve-Path -LiteralPath $Binary).Path
$evidencePath=[IO.Path]::GetFullPath((Join-Path (Get-Location) $Evidence))
New-Item -ItemType Directory -Force -Path (Join-Path $evidencePath 'config') | Out-Null
[IO.File]::WriteAllText((Join-Path $evidencePath 'config/settings.json'),'{"cache_mib":0,"accounts":[],"close_to_tray":false}')
$env:LUMA_TEST_ROOT=$evidencePath
$stdout=Join-Path $evidencePath 'performance.stdout';$stderr=Join-Path $evidencePath 'performance.stderr'
$arguments=if($Fluid){@('--design-preview','--resource-test')}else{@('--performance-diagnostics')}
if($Fluid){$env:LUMA_PREVIEW_HTML='1';$env:LUMA_RESOURCE_REPORT=Join-Path $evidencePath 'resource-lifecycle.json'}
# Start-Process hidden keeps a Qt scene unexposed. Direct process creation uses
# the requested visible native window and inherited standard stream pipes.
if($Fluid){
 $launch=[Diagnostics.ProcessStartInfo]::new();$launch.FileName=$binaryPath;$launch.Arguments=$arguments -join ' ';$launch.UseShellExecute=$false;$launch.RedirectStandardOutput=$true;$launch.RedirectStandardError=$true;$launch.CreateNoWindow=$true
 $process=[Diagnostics.Process]::Start($launch)
 $stdoutTask=$process.StandardOutput.ReadToEndAsync();$stderrTask=$process.StandardError.ReadToEndAsync()
}else{$process=Start-Process -FilePath $binaryPath -ArgumentList $arguments -PassThru -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr}
$null=$process.Handle # Preserve exit status in Windows PowerShell 5.1.
$timer=[Diagnostics.Stopwatch]::StartNew();$samples=@();$previous=@{};$last=0.0;$lastPhase='';$phaseStarted=0.0
try{
    while(-not $process.HasExited){
        Start-Sleep -Seconds 5
        if($process.HasExited){break}
        if($timer.Elapsed.TotalSeconds -gt $(if($Fluid){215}else{155})){throw 'Windows performance fixture timed out'}
        $ids=[Collections.Generic.HashSet[int]]::new();[void]$ids.Add($process.Id)
        $all=@(Get-CimInstance Win32_Process -Property ProcessId,ParentProcessId)
        do{$before=$ids.Count;foreach($item in $all){if($ids.Contains([int]$item.ParentProcessId)){[void]$ids.Add([int]$item.ProcessId)}}}while($ids.Count -gt $before)
        $members=@(Get-Process -Id ([int[]]$ids) -ErrorAction SilentlyContinue)
        $counters=@(Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Property IDProcess,WorkingSetPrivate | Where-Object {$ids.Contains([int]$_.IDProcess)})
        if($process.HasExited){break} # Do not record a zero-memory post-exit sample.
        $private=($counters | Measure-Object WorkingSetPrivate -Sum).Sum
        $commit=($members | Measure-Object PrivateMemorySize64 -Sum).Sum
        $working=($members | Measure-Object WorkingSet64 -Sum).Sum
        $now=$timer.Elapsed.TotalSeconds;$delta=0.0;$next=@{}
        foreach($member in $members){$value=$member.TotalProcessorTime.TotalSeconds;$next[$member.Id]=$value;if($previous.ContainsKey($member.Id)){$delta+=[Math]::Max(0,$value-$previous[$member.Id])}}
        # Get-Content decorates strings with PSDrive/Provider metadata. Windows
        # PowerShell 5 serializes that object graph (unlike PowerShell 7), which
        # can exhaust memory rather than write this small report. Copy only the
        # selected text to keep every sample plain; Get-Content permits sharing
        # the file while the fixture is still writing it.
        $phaseLine=if($Fluid){if(Test-Path (Join-Path $evidencePath 'phase.json')){(Get-Content (Join-Path $evidencePath 'phase.json') -Raw|ConvertFrom-Json).phase}else{''}}else{Get-Content -LiteralPath $stdout|Where-Object{$_ -match '^PERF '}|Select-Object -Last 1}
        $phase=if($null -eq $phaseLine){''}else{[string]::new($phaseLine.ToCharArray())}
        if($phase -ne $lastPhase){$phaseStarted=$now;$lastPhase=$phase}
        $cpu=if($last -gt 0){[Math]::Round(100*$delta/($now-$last),3)}else{$null}
        $details=@(foreach($member in $members){$counter=$counters|Where-Object{[int]$_.IDProcess -eq $member.Id}|Select-Object -First 1;@{id=$member.Id;name=$member.ProcessName;private_working_set_bytes=[long]$counter.WorkingSetPrivate;private_commit_bytes=$member.PrivateMemorySize64;working_set_bytes=$member.WorkingSet64}})
        $samples+=@{elapsed_seconds=[Math]::Round($now,2);phase=$phase;settled=($now-$phaseStarted -ge 8);processes=$members.Count;members=$details;private_working_set_bytes=[long]$private;private_commit_bytes=[long]$commit;working_set_sum_bytes=[long]$working;cpu_one_core_percent=$cpu;cpu_machine_percent=if($null -ne $cpu){[Math]::Round($cpu/[Environment]::ProcessorCount,4)}else{$null}}
        Write-Output ('SAMPLE {0:N1}s {1}: {2:N2} MiB private WS, {3} processes' -f $now,$phase,($private/1MB),$members.Count)
        $previous=$next;$last=$now
    }
    $process.WaitForExit()
    if($Fluid){[IO.File]::WriteAllText($stdout,$stdoutTask.GetAwaiter().GetResult());[IO.File]::WriteAllText($stderr,$stderrTask.GetAwaiter().GetResult())}
    if((Test-Path (Join-Path $evidencePath 'cache')) -and (Get-ChildItem (Join-Path $evidencePath 'cache') -File -Recurse | Select-Object -First 1)){throw 'Performance fixture persisted a zero-budget mail cache'}
    $phases=@{};foreach($group in ($samples|Where-Object{$_.settled -and $null -ne $_.cpu_one_core_percent}|Group-Object{$_['phase']})){$rows=@($group.Group | ForEach-Object {[pscustomobject]$_});$phases[$group.Name]=@{private_working_set_mib=[Math]::Round(($rows|Measure-Object private_working_set_bytes -Average).Average/1MB,3);private_commit_mib=[Math]::Round(($rows|Measure-Object private_commit_bytes -Average).Average/1MB,3);working_set_sum_mib=[Math]::Round(($rows|Measure-Object working_set_sum_bytes -Average).Average/1MB,3);cpu_one_core_percent=[Math]::Round(($rows|Measure-Object cpu_one_core_percent -Average).Average,3);samples=$rows.Count}}
    $report=@{binary_sha256=(Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash.ToLower();exit_code=$process.ExitCode;metric='Windows private working set and private commit; not Linux PSS';includes='Luma and recursively discovered child renderer/helper processes';cpu='Settled per-process CPU deltas, one-core and whole-machine percentages; short-lived exited helpers can be missed';phases=$phases;samples=$samples}
    $report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidencePath 'windows-performance.json') -Encoding UTF8
    Get-Content -LiteralPath $stdout
    $samples | Group-Object {$_['phase']} | ForEach-Object {$group=$_.Group | Select-Object -Last 1;[pscustomobject]@{Phase=$_.Name;Processes=$group.processes;PrivateWorkingSetMiB=[Math]::Round($group.private_working_set_bytes/1MB,2);CPUOneCorePercent=$group.cpu_one_core_percent}} | Format-Table
    if($process.ExitCode -ne 0){Get-Content -LiteralPath $stderr -TotalCount 25;throw 'Windows performance fixture failed; partial counters were retained'}
}finally{
    if(-not $process.HasExited){Stop-Process -Id $process.Id}
    foreach($name in $previousEnvironment.Keys){[Environment]::SetEnvironmentVariable($name,$previousEnvironment[$name],'Process')}
}

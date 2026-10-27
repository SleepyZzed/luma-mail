param([Parameter(Mandatory=$true)][string]$Binary,[Parameter(Mandatory=$true)][string]$Evidence,[switch]$Release,[string[]]$Kinds)
$ErrorActionPreference='Stop'
$binaryPath=(Resolve-Path -LiteralPath $Binary).Path
$evidencePath=[IO.Path]::GetFullPath((Join-Path (Get-Location) $Evidence))
New-Item -ItemType Directory -Force -Path $evidencePath | Out-Null
$cases=if($Kinds){$Kinds}elseif($Release){@('smoke','render')}else{@('compose','presentation','list','signature','render','windows-shell')}
foreach($kind in $cases){
    $root=Join-Path $evidencePath $kind
    New-Item -ItemType Directory -Force -Path (Join-Path $root 'config') | Out-Null
    [IO.File]::WriteAllText((Join-Path $root 'config/settings.json'),'{"cache_mib":0,"accounts":[],"theme":"noctalia"}')
    $palette="mode = `"dark`"`nbackground = `"#21191F`"`nsurface = `"#281E25`"`ntext = `"#F2EDF1`"`nmuted = `"#ADA2AC`"`naccent = `"#F2A4CF`"`non_accent = `"#321426`"`noutline = `"#44353F`"`nhover = `"#352631`"`n"
    [IO.File]::WriteAllText((Join-Path $root 'config/noctalia.toml'),$palette)
    $env:LUMA_TEST_ROOT=$root
    $env:LUMA_SMOKE_SCREENSHOT=Join-Path $evidencePath ($kind+'.png')
    $env:LUMA_SIGNATURE_SCREENSHOT=Join-Path $evidencePath ($kind+'.png')
    $arguments=if($kind -eq 'smoke'){@('--smoke-test','--desktop-diagnostics')}elseif($kind -eq 'render'){@('--render-diagnostics')}else{@('--design-preview',('--'+$kind+'-diagnostics'))}
    $stdout=Join-Path $evidencePath ($kind+'.stdout');$stderr=Join-Path $evidencePath ($kind+'.stderr')
    $process=Start-Process -FilePath $binaryPath -ArgumentList $arguments -PassThru -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    if(-not $process.WaitForExit(40000)){Stop-Process -Id $process.Id;Get-Content -LiteralPath $stdout;Get-Content -LiteralPath $stderr -TotalCount 20;throw "$kind Windows diagnostics timed out"}
    Get-Content -LiteralPath $stdout
    if($process.ExitCode -ne 0){Get-Content -LiteralPath $stderr -TotalCount 30;throw "$kind Windows diagnostics failed: $($process.ExitCode)"}
    if((Test-Path (Join-Path $root 'cache')) -and (Get-ChildItem (Join-Path $root 'cache') -File -Recurse | Select-Object -First 1)){throw "$kind persisted files with a zero mail-cache budget"}
    if(Test-Path (Join-Path $root 'config/compose')){throw "$kind created a local draft archive"}
}
Remove-Item Env:LUMA_TEST_ROOT -ErrorAction SilentlyContinue
Remove-Item Env:LUMA_SMOKE_SCREENSHOT -ErrorAction SilentlyContinue
Remove-Item Env:LUMA_SIGNATURE_SCREENSHOT -ErrorAction SilentlyContinue
Write-Output 'PASS native Windows acceptance and zero retained mail cache; source/attachment/local-draft storage remains online'

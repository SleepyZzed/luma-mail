$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'windows-process-tree.ps1')
function Fixture([int]$id,[int]$parent,[object]$born){[pscustomobject]@{ProcessId=$id;ParentProcessId=$parent;CreationDate=$born}}
$start=[DateTime]::UtcNow
$processes=@(
    (Fixture 100 1 $start),
    (Fixture 200 100 $start.AddSeconds(1)),
    (Fixture 300 200 $start.AddSeconds(2)),
    (Fixture 400 300 $start.AddSeconds(3)),
    (Fixture 500 200 $start.AddSeconds(-10)), # orphan whose parent ID was reused
    (Fixture 501 500 $start.AddSeconds(-5)),
    (Fixture 600 300 $start.AddMilliseconds(1500)), # newer than root, older than its claimed parent
    (Fixture 700 1 $start.AddSeconds(4)),
    (Fixture 800 200 $start.AddSeconds(1)), # equal timestamps are valid
    (Fixture 900 200 $null)
)
$ids=Get-LumaProcessTree -RootId 100 -Processes $processes
if(($ids | Sort-Object) -join ',' -ne '100,200,300,400,800'){throw 'Process-tree ancestry accepted an orphan or lost a real descendant'}
$reverse=@($processes);[array]::Reverse($reverse)
$other=Get-LumaProcessTree -RootId 100 -Processes $reverse
if(($other | Sort-Object) -join ',' -ne '100,200,300,400,800'){throw 'Process-tree ancestry depends on WMI enumeration order'}
Write-Output 'Windows process ancestry: descendants, reused IDs, missing dates and unordered input passed'

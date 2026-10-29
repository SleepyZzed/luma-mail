# A Windows orphan retains its numeric ParentProcessId after the parent exits.
# That ID can later be reused by Luma or one of its helpers. Creation dates
# distinguish genuine descendants from unrelated, older orphaned processes.
function Get-LumaProcessTree {
    param([int]$RootId,[object[]]$Processes)
    $byId=@{};foreach($item in $Processes){$byId[[int]$item.ProcessId]=$item}
    $ids=[Collections.Generic.HashSet[int]]::new();[void]$ids.Add($RootId)
    do {
        $before=$ids.Count
        foreach($item in $Processes){
            $parent=$byId[[int]$item.ParentProcessId]
            if($parent -and $ids.Contains([int]$item.ParentProcessId) -and $null -ne $item.CreationDate -and $null -ne $parent.CreationDate -and $item.CreationDate -ge $parent.CreationDate){[void]$ids.Add([int]$item.ProcessId)}
        }
    } while($ids.Count -gt $before)
    return ,$ids
}

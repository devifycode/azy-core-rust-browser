param([int]$BrowserPid)
$processRows = Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId
$owned = [System.Collections.Generic.HashSet[int]]::new()
[void]$owned.Add($BrowserPid)
do {
    $added = $false
    foreach ($row in $processRows) {
        if ($owned.Contains([int]$row.ParentProcessId) -and $owned.Add([int]$row.ProcessId)) { $added = $true }
    }
} while ($added)
$working = 0L
$private = 0L
$cpu = 0.0
$count = 0
foreach ($processNumber in $owned) {
    $item = Get-Process -Id $processNumber -ErrorAction SilentlyContinue
    if ($item) { $count++; $working += $item.WorkingSet64; $private += $item.PrivateMemorySize64; $cpu += $item.TotalProcessorTime.TotalSeconds }
}
[ordered]@{ processes=$count; workingSetMiB=[math]::Round($working/1MB,1); privateMiB=[math]::Round($private/1MB,1); cpuSeconds=$cpu } | ConvertTo-Json -Compress

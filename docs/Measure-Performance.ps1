param(
    [ValidateRange(2, 3600)][int]$DurationSeconds = 120,
    [ValidateRange(1, 30)][int]$IntervalSeconds = 2,
    [string]$Label = 'visible-30fps',
    [string]$ProcessName = 'time-flows-away-native',
    [string]$OutputPath = '.\performance.csv'
)
$ErrorActionPreference = 'Stop'
$rows = [System.Collections.Generic.List[object]]::new()
$previous = @{}
$previousAt = [DateTimeOffset]::Now
$logicalProcessors = [Environment]::ProcessorCount
$until = $previousAt.AddSeconds($DurationSeconds)
$gpuAvailable = $true
while ([DateTimeOffset]::Now -lt $until) {
    $now = [DateTimeOffset]::Now
    $processes = @(Get-Process -Name $ProcessName -ErrorAction SilentlyContinue)
    $cpuSeconds = 0.0
    $workingSet = 0L
    $privateBytes = 0L
    $current = @{}
    foreach ($process in $processes) {
        $id = $process.Id
        $total = $process.TotalProcessorTime.TotalSeconds
        $current[$id] = $total
        if ($previous.ContainsKey($id)) { $cpuSeconds += [Math]::Max(0, $total - $previous[$id]) }
        $workingSet += $process.WorkingSet64
        $privateBytes += $process.PrivateMemorySize64
    }
    $elapsed = ($now - $previousAt).TotalSeconds
    $cpu = if ($elapsed -gt 0 -and $previous.Count -gt 0) { 100 * $cpuSeconds / $elapsed / $logicalProcessors } else { $null }
    $gpu = $null
    if ($gpuAvailable -and $processes.Count -gt 0) {
        try {
            $ids = @($processes.Id)
            $engines = @{}
            foreach ($engine in Get-CimInstance Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine) {
                if ($engine.Name -match '^pid_(\d+)_(.+)$' -and $ids -contains [int]$Matches[1]) {
                    $key = $Matches[2]
                    if (-not $engines.ContainsKey($key)) { $engines[$key] = 0.0 }
                    $engines[$key] += [double]$engine.UtilizationPercentage
                }
            }
            # Maximum busy engine, with the app's processes summed per engine.
            # This is not a sum of parallel engines and cannot exceed 100%.
            if ($engines.Count -gt 0) { $gpu = [Math]::Min(100, ($engines.Values | Measure-Object -Maximum).Maximum) }
        } catch { $gpuAvailable = $false; Write-Warning 'GPU counters unavailable; GPU columns stay empty.' }
    }
    $rows.Add([pscustomobject]@{
        Time = $now.ToString('o'); Label = $Label; Processes = $processes.Count
        CPUPercent = $cpu; WorkingSetMiB = $workingSet / 1MB
        PrivateMiB = $privateBytes / 1MB; GPUBusiestEnginePercent = $gpu
    })
    $previous = $current
    $previousAt = $now
    Start-Sleep -Seconds $IntervalSeconds
}
if (($rows | Measure-Object -Property Processes -Maximum).Maximum -eq 0) {
    throw "No running process named $ProcessName was found. Start the app before measuring."
}
$rows | Export-Csv -LiteralPath $OutputPath -NoTypeInformation -Encoding UTF8
$rows | Measure-Object CPUPercent,WorkingSetMiB,PrivateMiB,GPUBusiestEnginePercent -Average -Maximum | Format-Table -AutoSize
Write-Host "Saved: $OutputPath (CPU normalized across $logicalProcessors logical processors; empty GPU means unavailable)."

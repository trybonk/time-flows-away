param(
    [ValidateRange(2, 7200)][int]$DurationSeconds = 120,
    [ValidateRange(1, 30)][int]$IntervalSeconds = 1,
    [string]$Label = 'visible-30fps',
    [string]$ProcessName = 'time-flows-away-native',
    [string]$OutputPath = '.\performance.csv',
    [switch]$SkipGpu
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/Performance-Helpers.ps1"
$rows = [System.Collections.Generic.List[object]]::new()
$previous = @{}
$roles = @{}
$clock = [Diagnostics.Stopwatch]::StartNew()
$previousAt = 0.0
$logicalProcessors = [Environment]::ProcessorCount
$gpuAvailable = -not $SkipGpu
while ($clock.Elapsed.TotalSeconds -lt $DurationSeconds) {
    $processes = @(Get-Process -Name $ProcessName -ErrorAction SilentlyContinue)
    $samples = @()
    $current = @{}
    $engines = @{}
    $gpuMemory = @{}
    if ($gpuAvailable -and $processes.Count -gt 0) {
        try {
            $ids = @($processes.Id)
            foreach ($engine in Get-CimInstance Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine) {
                if ($engine.Name -match '^pid_(\d+)_(.+)$' -and $ids -contains [int]$Matches[1]) {
                    $pidKey = [int]$Matches[1]; $engineKey = $Matches[2]
                    if (-not $engines.ContainsKey($pidKey)) { $engines[$pidKey] = @{} }
                    $engines[$pidKey][$engineKey] = [double]$engine.UtilizationPercentage
                }
            }
            foreach ($memory in Get-CimInstance Win32_PerfFormattedData_GPUPerformanceCounters_GPUProcessMemory) {
                if ($memory.Name -match '^pid_(\d+)_' -and $ids -contains [int]$Matches[1]) {
                    $pidKey = [int]$Matches[1]
                    if (-not $gpuMemory.ContainsKey($pidKey)) { $gpuMemory[$pidKey] = @(0.0, 0.0) }
                    $gpuMemory[$pidKey][0] += [double]$memory.DedicatedUsage / 1MB
                    $gpuMemory[$pidKey][1] += [double]$memory.SharedUsage / 1MB
                }
            }
        } catch {
            $gpuAvailable = $false; $engines = @{}; $gpuMemory = @{}
            Write-Warning 'GPU counters unavailable; GPU columns stay empty.'
        }
    }
    $at = $clock.Elapsed.TotalSeconds
    $elapsed = $at - $previousAt
    $now = [DateTimeOffset]::Now.ToString('o')
    foreach ($process in $processes) {
        try {
            $id = $process.Id
            $identity = "$id/$($process.StartTime.ToUniversalTime().Ticks)"
            $total = $process.TotalProcessorTime.TotalSeconds
            $current[$identity] = $total
            $delta = if ($previous.ContainsKey($identity)) { [Math]::Max(0.0, $total - $previous[$identity]) } else { $null }
            $cpu = if ($null -ne $delta) { Get-NormalizedCpu $delta $elapsed $logicalProcessors } else { $null }
            if (-not $roles.ContainsKey($identity)) {
                $roles[$identity] = 'unknown'
                if ($ProcessName -eq 'time-flows-away-native' -and $env:OS -eq 'Windows_NT') {
                    try {
                        $line = (Get-CimInstance Win32_Process -Filter "ProcessId=$id").CommandLine
                        $roles[$identity] = if ($line -match '(?:^|\s)--settings(?:\s|$)') { 'settings' } elseif ($line -match '(?:^|\s)--preview(?:\s|$)') { 'preview' } elseif ($line -match '(?:^|\s)--wallpaper-worker(?:\s|$)') { 'renderer' } else { 'supervisor' }
                    } catch { Write-Warning "Cannot identify role of PID $id." }
                }
            }
            $gpu = if ($engines.ContainsKey($id)) { Get-BusiestGpuEngine @($engines[$id].Values) } else { $null }
            $dedicated = if ($gpuMemory.ContainsKey($id)) { $gpuMemory[$id][0] } else { $null }
            $shared = if ($gpuMemory.ContainsKey($id)) { $gpuMemory[$id][1] } else { $null }
            $sample = [pscustomobject]@{
                Time = $now; Label = $Label; Role = $roles[$identity]; PID = $id; Processes = 1
                IntervalSeconds = $elapsed; CpuSeconds = $total; SampleCpuSeconds = $delta
                CPUPercent = $cpu; CPUComplete = ($null -ne $cpu)
                WorkingSetMiB = $process.WorkingSet64 / 1MB; PrivateMiB = $process.PrivateMemorySize64 / 1MB
                Handles = $process.HandleCount; Threads = $process.Threads.Count
                GPU3DPercent = Get-GpuEngineUsage $engines[$id] 3D
                GPUCopyPercent = Get-GpuEngineUsage $engines[$id] Copy
                GPUVideoDecodePercent = Get-GpuEngineUsage $engines[$id] VideoDecode
                GPUVideoEncodePercent = Get-GpuEngineUsage $engines[$id] VideoEncode
                GPUBusiestEnginePercent = $gpu; GPUDedicatedMiB = $dedicated; GPUSharedMiB = $shared
            }
            $rows.Add($sample); $samples += $sample
        } catch { Write-Warning "PID $($process.Id) exited while sampling; skipped." }
    }
    $complete = $samples.Count -gt 0 -and @($samples | Where-Object { -not $_.CPUComplete }).Count -eq 0
    $totalEngines = @{}
    foreach ($perPid in $engines.Values) {
        foreach ($key in $perPid.Keys) {
            if (-not $totalEngines.ContainsKey($key)) { $totalEngines[$key] = 0.0 }
            $totalEngines[$key] += $perPid[$key]
        }
    }
    $total = [pscustomobject]@{
        Time = $now; Label = $Label; Role = 'TOTAL'; PID = 0; Processes = $samples.Count
        IntervalSeconds = $elapsed; CpuSeconds = ($samples | Measure-Object CpuSeconds -Sum).Sum
        SampleCpuSeconds = $(if ($complete) { ($samples | Measure-Object SampleCpuSeconds -Sum).Sum } else { $null })
        CPUPercent = $(if ($complete) { ($samples | Measure-Object CPUPercent -Sum).Sum } else { $null }); CPUComplete = $complete
        WorkingSetMiB = ($samples | Measure-Object WorkingSetMiB -Sum).Sum; PrivateMiB = ($samples | Measure-Object PrivateMiB -Sum).Sum
        Handles = ($samples | Measure-Object Handles -Sum).Sum; Threads = ($samples | Measure-Object Threads -Sum).Sum
        GPU3DPercent = Get-GpuEngineUsage $totalEngines 3D
        GPUCopyPercent = Get-GpuEngineUsage $totalEngines Copy
        GPUVideoDecodePercent = Get-GpuEngineUsage $totalEngines VideoDecode
        GPUVideoEncodePercent = Get-GpuEngineUsage $totalEngines VideoEncode
        GPUBusiestEnginePercent = Get-BusiestGpuEngine @($totalEngines.Values)
        GPUDedicatedMiB = $(if ($gpuMemory.Count -gt 0) { ($samples | Measure-Object GPUDedicatedMiB -Sum).Sum } else { $null })
        GPUSharedMiB = $(if ($gpuMemory.Count -gt 0) { ($samples | Measure-Object GPUSharedMiB -Sum).Sum } else { $null })
    }
    $rows.Add($total)
    $previous = $current; $previousAt = $at
    $remaining = [Math]::Min([double]$IntervalSeconds, [double]$DurationSeconds - $clock.Elapsed.TotalSeconds)
    if ($remaining -gt 0) { Start-Sleep -Milliseconds ([int]($remaining * 1000)) }
}
if (@($rows | Where-Object { $_.Role -eq 'TOTAL' -and $_.Processes -gt 0 }).Count -eq 0) {
    throw "No running process named $ProcessName was found. Start the app before measuring."
}
Export-InvariantCsv $rows $OutputPath
$totals = @($rows | Where-Object { $_.Role -eq 'TOTAL' })
foreach ($property in 'CPUPercent','WorkingSetMiB','PrivateMiB','GPUBusiestEnginePercent') {
    @($totals | Where-Object { $null -ne $_.$property }) | Measure-Object -Property $property -Average -Maximum | Format-Table -AutoSize
}
Write-Host "Saved: $OutputPath (per-PID and TOTAL; CPU normalized across $logicalProcessors logical processors; empty means unavailable)."

# Shared arithmetic/CSV code; works in Windows PowerShell 5.1 and PowerShell 7.
function Get-NormalizedCpu {
    param([double]$DeltaSeconds, [double]$ElapsedSeconds, [int]$LogicalProcessors)
    if ($ElapsedSeconds -le 0 -or $LogicalProcessors -le 0) { return $null }
    100.0 * [Math]::Max(0.0, $DeltaSeconds) / $ElapsedSeconds / $LogicalProcessors
}
function Get-BusiestGpuEngine {
    param($Values)
    if (@($Values).Count -eq 0) { return $null }
    [Math]::Min(100.0, [double](($Values | Measure-Object -Maximum).Maximum))
}
function Get-GpuEngineUsage {
    param($Engines, [string]$Kind)
    if ($null -eq $Engines) { return $null }
    $values = @($Engines.Keys | Where-Object { $_ -match ('_engtype_' + [regex]::Escape($Kind) + '$') } | ForEach-Object { $Engines[$_] })
    Get-BusiestGpuEngine $values
}
function Export-InvariantCsv {
    param($Rows, [string]$Path)
    $invariant = [Globalization.CultureInfo]::InvariantCulture
    $converted = foreach ($row in $Rows) {
        $fields = [ordered]@{}
        foreach ($property in $row.PSObject.Properties) {
            $value = $property.Value
            if ($value -is [double] -or $value -is [single] -or $value -is [decimal]) {
                $value = $value.ToString('G', $invariant)
            }
            $fields[$property.Name] = $value
        }
        [pscustomobject]$fields
    }
    $lines = @($converted | ConvertTo-Csv -NoTypeInformation)
    for ($attempt = 0; $attempt -lt 10; $attempt++) {
        try {
            # Explicit BOM on both PS versions; Excel handles Unicode labels.
            [IO.File]::WriteAllLines([IO.Path]::GetFullPath($Path), [string[]]$lines, [Text.UTF8Encoding]::new($true))
            return
        } catch [IO.IOException] {
            if ($attempt -eq 9) { throw }
            Start-Sleep -Milliseconds 200
        }
    }
}

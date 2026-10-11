$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/Performance-Helpers.ps1"
if ([Math]::Abs((Get-NormalizedCpu 0.4 1.0 6) - (100.0 * 0.4 / 6)) -gt 0.000001) { throw 'Fractional CPU delta was rounded.' }
if ((Get-NormalizedCpu -0.25 1.0 6) -ne 0.0) { throw 'Negative delta was not clamped.' }
if ($null -ne (Get-NormalizedCpu 0.5 0.0 6)) { throw 'Zero duration is not unavailable.' }
if ((Get-BusiestGpuEngine @(0.25, 1.75)) -ne 1.75) { throw 'GPU engine values were rounded.' }
if ($null -ne (Get-BusiestGpuEngine @())) { throw 'Missing GPU counters became zero.' }
if ((Get-GpuEngineUsage @{'luid_1_engtype_3D' = 1.25; 'luid_1_engtype_Copy' = 22.5} 3D) -ne 1.25) { throw 'GPU engine kind regression.' }
$file = Join-Path ([IO.Path]::GetTempPath()) ('tfa-helper-' + [guid]::NewGuid().ToString() + '.csv')
Add-Type @'
using System.IO;
using System.Threading;
public static class TfaCsvLockTest {
    public static void ReleaseLater(FileStream stream) {
        var thread = new Thread(() => { Thread.Sleep(300); stream.Dispose(); });
        thread.IsBackground = true;
        thread.Start();
    }
}
'@
$old = [Threading.Thread]::CurrentThread.CurrentCulture
try {
    [Threading.Thread]::CurrentThread.CurrentCulture = [Globalization.CultureInfo]::GetCultureInfo('ru-RU')
    $held = [IO.File]::Open($file, [IO.FileMode]::OpenOrCreate, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
    [TfaCsvLockTest]::ReleaseLater($held)
    Export-InvariantCsv @([pscustomobject]@{ CPUPercent = 1.25; Label = 'проверка'; GPU = $null }) $file
    $raw = [IO.File]::ReadAllBytes($file)
    if ($raw[0] -ne 239 -or $raw[1] -ne 187 -or $raw[2] -ne 191) { throw 'CSV has no UTF-8 BOM.' }
    $row = Import-Csv $file
    if ($row.CPUPercent -ne '1.25' -or $row.Label -ne 'проверка' -or $row.GPU -ne '') { throw 'CSV culture/Unicode/unavailable value regression.' }
} finally {
    [Threading.Thread]::CurrentThread.CurrentCulture = $old
    Remove-Item $file -ErrorAction SilentlyContinue
}
Write-Host 'Performance arithmetic and CSV checks passed.'

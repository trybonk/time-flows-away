# Open the Time Flows Away tray menu, then run this script from another terminal.
# Reads only the native popup menu, including the MSAA bridge overlooked by UIA-only checks.
param([string]$OutputPath = '.\tray-accessibility.json')
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Requires Windows and an open native tray menu.' }
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, Accessibility
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class TfaMenuAccess {
    public delegate bool EnumProc(IntPtr hwnd, IntPtr param);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr param);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr hwnd, StringBuilder text, int max);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("oleacc.dll")] public static extern int AccessibleObjectFromWindow(IntPtr hwnd, uint objectId, ref Guid iid, [MarshalAs(UnmanagedType.Interface)] out object accessible);
}
'@
$windows = [Collections.Generic.List[IntPtr]]::new()
$callback = [TfaMenuAccess+EnumProc]{ param($hwnd, $unused)
    $class = [Text.StringBuilder]::new(128)
    [void][TfaMenuAccess]::GetClassName($hwnd, $class, 128)
    if ($class.ToString() -eq '#32768' -and [TfaMenuAccess]::IsWindowVisible($hwnd)) { $windows.Add($hwnd) }
    return $true
}
[void][TfaMenuAccess]::EnumWindows($callback, [IntPtr]::Zero)
if ($windows.Count -eq 0) { throw 'No visible popup menu found. Keep the tray menu open while running.' }
function Read-UiaTree($Element, $Walker, [int]$Depth = 0) {
    if ($Depth -gt 6 -or $null -eq $Element) { return }
    try {
        [pscustomobject]@{ depth = $Depth; name = $Element.Current.Name; type = $Element.Current.ControlType.ProgrammaticName; class = $Element.Current.ClassName }
        $child = $Walker.GetFirstChild($Element)
        for ($i = 0; $i -lt 64 -and $null -ne $child; $i++) {
            Read-UiaTree $child $Walker ($Depth + 1)
            $child = $Walker.GetNextSibling($child)
        }
    } catch { [pscustomobject]@{ error = $_.Exception.Message } }
}
$results = foreach ($hwnd in $windows) {
    $element = [System.Windows.Automation.AutomationElement]::FromHandle($hwnd)
    $msaa = foreach ($objectId in @(0, -4, -3)) {
        $iid = [Accessibility.IAccessible].GUID; $object = $null
        $unsigned = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int]$objectId), 0)
        $code = [TfaMenuAccess]::AccessibleObjectFromWindow($hwnd, $unsigned, [ref]$iid, [ref]$object)
        if ($code -ne 0 -or $null -eq $object) { [pscustomobject]@{ objectId = $objectId; hresult = $code }; continue }
        try {
            $accessible = [Accessibility.IAccessible]$object
            for ($i = 0; $i -le [Math]::Min(64, $accessible.accChildCount); $i++) {
                try { [pscustomobject]@{ objectId = $objectId; child = $i; name = $accessible.get_accName($i); role = $accessible.get_accRole($i); state = $accessible.get_accState($i); action = $accessible.get_accDefaultAction($i) } } catch { [pscustomobject]@{ objectId = $objectId; child = $i; error = $_.Exception.Message } }
            }
        } finally { [void][Runtime.InteropServices.Marshal]::ReleaseComObject($object) }
    }
    [pscustomobject]@{ hwnd = $hwnd.ToInt64(); raw = @(Read-UiaTree $element ([System.Windows.Automation.TreeWalker]::RawViewWalker)); control = @(Read-UiaTree $element ([System.Windows.Automation.TreeWalker]::ControlViewWalker)); msaa = @($msaa) }
}
$results | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $OutputPath -Encoding UTF8
Write-Host "Saved: $OutputPath. Also verify Narrator names/actions and arrow-key navigation."

param(
    [Parameter(Mandatory = $true)][int]$AppProcessId,
    [ValidateSet('Snapshot', 'Theme')][string]$Action = 'Snapshot',
    [ValidateRange(-1, 1)][int]$LightTheme = -1
)

$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class NativeCheckWindow {
    public delegate bool EnumProc(IntPtr window, IntPtr parameter);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct MonitorInfo { public int Size; public Rect Monitor, Work; public int Flags; }
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr parameter);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr window, StringBuilder text, int length);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr window, uint flags);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern bool GetMonitorInfo(IntPtr monitor, ref MonitorInfo info);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr SendMessageTimeout(IntPtr window, uint message, IntPtr parameter, string text, uint flags, uint timeout, out IntPtr result);
}
'@

# 本探针按物理像素采集窗口与显示器，报告中同时保留 DPI。
$null = [NativeCheckWindow]::SetProcessDpiAwarenessContext([IntPtr](-4))
$themeKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize'
if ($Action -eq 'Theme') {
    if ($LightTheme -eq -1) {
        Remove-ItemProperty -Path $themeKey -Name AppsUseLightTheme -ErrorAction SilentlyContinue
    } else {
        $null = New-Item -Path $themeKey -Force
        $null = New-ItemProperty -Path $themeKey -Name AppsUseLightTheme -Value $LightTheme -PropertyType DWord -Force
    }
    $result = [IntPtr]::Zero
    $null = [NativeCheckWindow]::SendMessageTimeout([IntPtr]0xffff, 0x001a, [IntPtr]::Zero, 'ImmersiveColorSet', 2, 3000, [ref]$result)
}

$windows = [System.Collections.Generic.List[object]]::new()
$callback = [NativeCheckWindow+EnumProc] {
    param([IntPtr]$window, [IntPtr]$parameter)
    $owner = [uint32]0
    $null = [NativeCheckWindow]::GetWindowThreadProcessId($window, [ref]$owner)
    if ($owner -ne $AppProcessId) { return $true }
    $title = [System.Text.StringBuilder]::new(512)
    $null = [NativeCheckWindow]::GetWindowText($window, $title, $title.Capacity)
    $outer = [NativeCheckWindow+Rect]::new()
    $client = [NativeCheckWindow+Rect]::new()
    if (-not [NativeCheckWindow]::GetWindowRect($window, [ref]$outer)) { throw 'GetWindowRect 失败' }
    if (-not [NativeCheckWindow]::GetClientRect($window, [ref]$client)) { throw 'GetClientRect 失败' }
    $monitor = [NativeCheckWindow+MonitorInfo]::new()
    $monitor.Size = [Runtime.InteropServices.Marshal]::SizeOf($monitor)
    $handle = [NativeCheckWindow]::MonitorFromWindow($window, 2)
    if (-not [NativeCheckWindow]::GetMonitorInfo($handle, [ref]$monitor)) { throw 'GetMonitorInfo 失败' }
    $windows.Add(@{
        handle = $window.ToInt64().ToString(); title = $title.ToString()
        visible = [NativeCheckWindow]::IsWindowVisible($window)
        dpi = [NativeCheckWindow]::GetDpiForWindow($window)
        outer = @{ x = $outer.Left; y = $outer.Top; width = $outer.Right - $outer.Left; height = $outer.Bottom - $outer.Top }
        client = @{ width = $client.Right; height = $client.Bottom }
        workArea = @{ x = $monitor.Work.Left; y = $monitor.Work.Top; width = $monitor.Work.Right - $monitor.Work.Left; height = $monitor.Work.Bottom - $monitor.Work.Top }
    })
    return $true
}
if (-not [NativeCheckWindow]::EnumWindows($callback, [IntPtr]::Zero)) { throw 'EnumWindows 失败' }
$theme = Get-ItemProperty -Path $themeKey -Name AppsUseLightTheme -ErrorAction SilentlyContinue
@{ windows = @($windows.ToArray()); lightTheme = $theme.AppsUseLightTheme } | ConvertTo-Json -Depth 6 -Compress

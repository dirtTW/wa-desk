# Capture a top-level window to a PNG so its rendered colors can be sampled.
# Usage: powershell -File scripts/_capture-window.ps1 -TitleMatch WhatsApp -Out C:\path\shot.png
param(
  [string]$TitleMatch = "WhatsApp",
  [string]$Out = "$env:TEMP\win.png"
)

Add-Type -AssemblyName System.Drawing

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win32Cap {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBlt, uint nFlags);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

$proc = Get-Process | Where-Object { $_.MainWindowTitle -and $_.MainWindowTitle -like "*$TitleMatch*" } | Select-Object -First 1
if (-not $proc) { Write-Output "NO_WINDOW"; exit 1 }
$h = $proc.MainWindowHandle
Write-Output ("process=" + $proc.ProcessName + " title=" + $proc.MainWindowTitle + " pid=" + $proc.Id)

[void][Win32Cap]::ShowWindow($h, 9)   # SW_RESTORE
[void][Win32Cap]::SetForegroundWindow($h)
Start-Sleep -Milliseconds 900

$r = New-Object Win32Cap+RECT
[void][Win32Cap]::GetWindowRect($h, [ref]$r)
$w = $r.Right - $r.Left
$ht = $r.Bottom - $r.Top
Write-Output ("rect=" + $r.Left + "," + $r.Top + " size=" + $w + "x" + $ht)

$bmp = New-Object System.Drawing.Bitmap($w, $ht)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
# 2 = PW_RENDERFULLCONTENT (needed for GPU-composited WebView2)
$ok = [Win32Cap]::PrintWindow($h, $hdc, 2)
$g.ReleaseHdc($hdc)
$g.Dispose()
if (-not $ok) {
  # Fall back to a screen BitBlt of the window rect.
  $g2 = [System.Drawing.Graphics]::FromImage($bmp)
  $g2.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($w, $ht)))
  $g2.Dispose()
  Write-Output "printwindow_failed_used_bitblt"
}
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Output ("saved=" + $Out + " bytes=" + (Get-Item $Out).Length)

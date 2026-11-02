# Sample pixel colors from a PNG. Usage:
#   powershell -File scripts/_sample-png.ps1 -Path shot.png -Points "rail:40,700;sidebar:600,600"
param(
  [Parameter(Mandatory = $true)][string]$Path,
  [Parameter(Mandatory = $true)][string]$Points
)
Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::FromFile($Path)
Write-Output ("image=" + $bmp.Width + "x" + $bmp.Height)
foreach ($p in $Points.Split(";")) {
  if ([string]::IsNullOrWhiteSpace($p)) { continue }
  $parts = $p.Split(":")
  $name = $parts[0]
  $xy = $parts[1].Split(",")
  $x = [int]$xy[0]; $y = [int]$xy[1]
  if ($x -lt 0 -or $y -lt 0 -or $x -ge $bmp.Width -or $y -ge $bmp.Height) {
    Write-Output ("{0,-22} ({1},{2}) OUT_OF_RANGE" -f $name, $x, $y); continue
  }
  $c = $bmp.GetPixel($x, $y)
  Write-Output ("{0,-22} ({1,4},{2,4})  #{3:X2}{4:X2}{5:X2}  rgb({3},{4},{5})  a={6}" -f $name, $x, $y, $c.R, $c.G, $c.B, $c.A)
}
$bmp.Dispose()

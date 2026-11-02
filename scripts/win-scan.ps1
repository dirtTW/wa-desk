# Print the runs of constant color along a horizontal or vertical line of a PNG.
# Usage: powershell -File scripts/_scan-line.ps1 -Path shot.png -Line "y=1050,800,1600"
param(
  [Parameter(Mandatory = $true)][string]$Path,
  [Parameter(Mandatory = $true)][string]$Line,
  [int]$MinRun = 3
)
Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::FromFile($Path)
$parts = $Line.Split(",")
$axis, $a0, $a1 = $parts[0].Split("=")[0], [int]$parts[0].Split("=")[1], 0
$from = [int]$parts[1]; $to = [int]$parts[2]
Write-Output ("image=" + $bmp.Width + "x" + $bmp.Height + "  line " + $Line)

function Hex([System.Drawing.Color]$c) { "#{0:X2}{1:X2}{2:X2}" -f $c.R, $c.G, $c.B }

$prev = $null; $runStart = $from; $runs = @()
for ($i = $from; $i -le $to; $i++) {
  if ($axis -eq "y") { $c = $bmp.GetPixel($i, $a0) } else { $c = $bmp.GetPixel($a0, $i) }
  $h = Hex $c
  if ($null -eq $prev) { $prev = $h; $runStart = $i; continue }
  if ($h -ne $prev) {
    if (($i - $runStart) -ge $MinRun) { $runs += [pscustomobject]@{ Hex = $prev; Start = $runStart; End = $i - 1; Len = $i - $runStart } }
    $prev = $h; $runStart = $i
  }
}
if (($to - $runStart + 1) -ge $MinRun) { $runs += [pscustomobject]@{ Hex = $prev; Start = $runStart; End = $to; Len = $to - $runStart + 1 } }
foreach ($r in $runs) { Write-Output ("  {0}  {1,5}..{2,-5} len={3}" -f $r.Hex, $r.Start, $r.End, $r.Len) }
$bmp.Dispose()

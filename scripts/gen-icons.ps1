Add-Type -AssemblyName System.Drawing

$out = Join-Path (Split-Path -Parent $PSScriptRoot) "apps\desktop\src-tauri\icons"
New-Item -ItemType Directory -Force -Path $out | Out-Null

function New-IconBitmap([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([System.Drawing.Color]::Transparent)

    $pad = [math]::Round($size * 0.06)
    $rect = New-Object System.Drawing.Rectangle $pad, $pad, ($size - 2 * $pad), ($size - 2 * $pad)
    $radius = [math]::Round($size * 0.24)

    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = $radius * 2
    $path.AddArc($rect.X, $rect.Y, $d, $d, 180, 90)
    $path.AddArc($rect.Right - $d, $rect.Y, $d, $d, 270, 90)
    $path.AddArc($rect.Right - $d, $rect.Bottom - $d, $d, $d, 0, 90)
    $path.AddArc($rect.X, $rect.Bottom - $d, $d, $d, 90, 90)
    $path.CloseFigure()

    $c1 = [System.Drawing.Color]::FromArgb(255, 37, 211, 102)
    $c2 = [System.Drawing.Color]::FromArgb(255, 0, 128, 105)
    $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, $c1, $c2, 45.0
    $g.FillPath($brush, $path)

    # white speech bubble
    $white = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 255, 255, 255))
    $bw = [math]::Round($size * 0.50)
    $bh = [math]::Round($size * 0.38)
    $bx = [math]::Round(($size - $bw) / 2)
    $by = [math]::Round(($size - $bh) / 2 - $size * 0.02)
    $bubble = New-Object System.Drawing.Rectangle $bx, $by, $bw, $bh
    $br = [math]::Round($size * 0.10)
    $bp = New-Object System.Drawing.Drawing2D.GraphicsPath
    $bd = $br * 2
    $bp.AddArc($bubble.X, $bubble.Y, $bd, $bd, 180, 90)
    $bp.AddArc($bubble.Right - $bd, $bubble.Y, $bd, $bd, 270, 90)
    $bp.AddArc($bubble.Right - $bd, $bubble.Bottom - $bd, $bd, $bd, 0, 90)
    $bp.AddArc($bubble.X, $bubble.Bottom - $bd, $bd, $bd, 90, 90)
    $bp.CloseFigure()
    $g.FillPath($white, $bp)

    $tail = New-Object System.Drawing.Drawing2D.GraphicsPath
    $tx = $bx + $bw * 0.20
    $ty = $by + $bh
    $tail.AddPolygon(@(
        (New-Object System.Drawing.Point ([int]$tx), ([int]$ty - 2)),
        (New-Object System.Drawing.Point ([int]($tx + $size * 0.12)), ([int]$ty - 2)),
        (New-Object System.Drawing.Point ([int]$tx), ([int]($ty + $size * 0.10)))
    ))
    $g.FillPath($white, $tail)

    # three green dots
    $dot = [math]::Round($size * 0.07)
    $gap = [math]::Round($size * 0.09)
    $cy = $by + $bh / 2 - $dot / 2
    $startX = $size / 2 - $gap
    $green = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 0, 128, 105))
    for ($i = 0; $i -lt 3; $i++) {
        $g.FillEllipse($green, [int]($startX + $i * $gap - $dot / 2), [int]$cy, [int]$dot, [int]$dot)
    }

    $g.Dispose()
    return $bmp
}

foreach ($spec in @(@(512, "icon.png"), @(256, "128x128@2x.png"), @(128, "128x128.png"), @(32, "32x32.png"))) {
    $size = $spec[0]
    $name = $spec[1]
    $bmp = New-IconBitmap $size
    $bmp.Save((Join-Path $out $name), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}

# Build a single-image .ico that embeds the 256px PNG (Vista+ ICO layout).
$pngPath = Join-Path $out "128x128@2x.png"
$png = [System.IO.File]::ReadAllBytes($pngPath)
$ico = New-Object System.IO.MemoryStream
$bw = New-Object System.IO.BinaryWriter $ico
$bw.Write([uint16]0)          # reserved
$bw.Write([uint16]1)          # type: icon
$bw.Write([uint16]1)          # image count
$bw.Write([byte]0)            # width 0 => 256
$bw.Write([byte]0)            # height 0 => 256
$bw.Write([byte]0)            # palette
$bw.Write([byte]0)            # reserved
$bw.Write([uint16]1)          # planes
$bw.Write([uint16]32)         # bpp
$bw.Write([uint32]$png.Length)
$bw.Write([uint32]22)         # offset
$bw.Write($png)
$bw.Flush()
[System.IO.File]::WriteAllBytes((Join-Path $out "icon.ico"), $ico.ToArray())
$bw.Dispose()

Get-ChildItem $out | Select-Object Name, Length

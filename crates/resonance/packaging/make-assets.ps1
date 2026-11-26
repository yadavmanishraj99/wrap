# Generates the MSIX tile / store logos into assets\ - the Resonance
# mark: a teal gradient tile with five white equalizer bars.
# package.ps1 runs this automatically when the PNGs are missing;
# the images are generated, never committed (they are binary).
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$assetsDir = Join-Path $PSScriptRoot "assets"
New-Item -ItemType Directory -Force -Path $assetsDir | Out-Null

function New-Tile([int]$w, [int]$h, [string]$outPath) {
    $ss = 4  # supersample, then downscale for smooth edges
    $W = $w * $ss; $H = $h * $ss
    $bmp = New-Object System.Drawing.Bitmap($W, $H)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias

    $top = [System.Drawing.Color]::FromArgb(5, 107, 92)    # #056B5C
    $bottom = [System.Drawing.Color]::FromArgb(3, 74, 63) # #034A3F
    $rect = New-Object System.Drawing.Rectangle(0, 0, $W, $H)
    $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        $rect, $top, $bottom, [System.Drawing.Drawing2D.LinearGradientMode]::Vertical)
    $g.FillRectangle($brush, $rect)

    $m = [Math]::Min($W, $H)
    $barW = $m * 0.105; $gap = $m * 0.062; $maxH = $m * 0.52
    $heights = @(0.42, 0.75, 1.0, 0.62, 0.36)
    $totalW = 5 * $barW + 4 * $gap
    $x0 = ($W - $totalW) / 2; $cy = $H / 2
    $white = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::White)
    for ($i = 0; $i -lt 5; $i++) {
        $bh = $maxH * $heights[$i]
        $x = $x0 + $i * ($barW + $gap)
        $y = $cy - $bh / 2
        $bar = New-Object System.Drawing.Drawing2D.GraphicsPath
        $bar.AddArc($x, $y, $barW, $barW, 180, 180)
        $bar.AddArc($x, $y + $bh - $barW, $barW, $barW, 0, 180)
        $bar.CloseFigure()
        $g.FillPath($white, $bar)
        $bar.Dispose()
    }

    $out = New-Object System.Drawing.Bitmap($w, $h)
    $g2 = [System.Drawing.Graphics]::FromImage($out)
    $g2.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g2.DrawImage($bmp, 0, 0, $w, $h)
    $out.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)

    $g.Dispose(); $g2.Dispose(); $bmp.Dispose(); $out.Dispose()
    $brush.Dispose(); $white.Dispose()
}

New-Tile 44  44  (Join-Path $assetsDir "Square44x44Logo.png")
New-Tile 50  50  (Join-Path $assetsDir "StoreLogo.png")
New-Tile 150 150 (Join-Path $assetsDir "Square150x150Logo.png")
New-Tile 310 150 (Join-Path $assetsDir "Wide310x150Logo.png")
New-Tile 310 310 (Join-Path $assetsDir "Square310x310Logo.png")
Write-Host "Tile assets written to $assetsDir"

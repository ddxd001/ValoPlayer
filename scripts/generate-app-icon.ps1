[CmdletBinding()]
param(
    [string]$PreviewDirectory = (Join-Path $PSScriptRoot '../../icon-preview')
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$projectDirectory = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$iconDirectory = Join-Path $projectDirectory 'src-tauri/icons'
$PreviewDirectory = [IO.Path]::GetFullPath($PreviewDirectory)
New-Item -ItemType Directory -Force -Path $iconDirectory, $PreviewDirectory | Out-Null

# Native geometry and colors match src-tauri/icons/icon.svg. Each raster frame
# is painted independently at 4x resolution, then reduced with alpha preserved.
function Get-IconColor([string]$Hex, [int]$Alpha = 255) {
    $color = [Drawing.ColorTranslator]::FromHtml($Hex)
    return [Drawing.Color]::FromArgb($Alpha, $color.R, $color.G, $color.B)
}

function New-RoundedTile([float]$X, [float]$Y, [float]$Width, [float]$Height, [float]$Radius) {
    $path = [Drawing.Drawing2D.GraphicsPath]::new()
    $diameter = 2 * $Radius
    $path.AddArc($X, $Y, $diameter, $diameter, 180, 90)
    $path.AddArc(($X + $Width - $diameter), $Y, $diameter, $diameter, 270, 90)
    $path.AddArc(($X + $Width - $diameter), ($Y + $Height - $diameter), $diameter, $diameter, 0, 90)
    $path.AddArc($X, ($Y + $Height - $diameter), $diameter, $diameter, 90, 90)
    $path.CloseFigure()
    return $path
}

function Draw-IconShape($Graphics, [double[][]]$Coordinates, [Drawing.Color]$Color) {
    $points = [Drawing.PointF[]]@($Coordinates | ForEach-Object { [Drawing.PointF]::new($_[0], $_[1]) })
    $brush = [Drawing.SolidBrush]::new($Color)
    try { $Graphics.FillPolygon($brush, $points) } finally { $brush.Dispose() }
}

function New-IconFrame([int]$Size) {
    $supersampling = 4
    $largeSize = $Size * $supersampling
    $large = [Drawing.Bitmap]::new($largeSize, $largeSize, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $large.SetResolution(96, 96)
    $graphics = [Drawing.Graphics]::FromImage($large)
    $graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $graphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $graphics.Clear([Drawing.Color]::Transparent)
    $graphics.ScaleTransform(($largeSize / 64), ($largeSize / 64))
    $tile = New-RoundedTile 0.5 0.5 63 63 13
    $fill = [Drawing.Drawing2D.LinearGradientBrush]::new([Drawing.RectangleF]::new(0, 0, 64, 64), (Get-IconColor '#202230'), (Get-IconColor '#0C0D14'), [float]45)
    $border = [Drawing.Pen]::new((Get-IconColor '#454653' 153), [float]0.65)
    try {
        $graphics.FillPath($fill, $tile)
        $graphics.DrawPath($border, $tile)
        $v = @(@(0, 0), @(12, 0), @(22, 29), @(32, 0), @(44, 0), @(27, 48), @(17, 48))
        $p = @(@(42, 0), @(64, 0), @(64, 25), @(53, 25), @(53, 48), @(41, 48), @(41, 10))
        $play = @(@(52, 7), @(59, 12), @(52, 18))
        foreach ($shape in @(@{ Points = $v; Color = Get-IconColor '#F4F5F8' }, @{ Points = $p; Color = Get-IconColor '#FF6575' }, @{ Points = $play; Color = Get-IconColor '#15161F' })) {
            $mapped = [double[][]]@($shape.Points | ForEach-Object { ,@((8 + ($_[0] * 0.75)), (14 + ($_[1] * 0.75))) })
            Draw-IconShape $graphics $mapped $shape.Color
        }
    } finally { $graphics.Dispose(); $fill.Dispose(); $border.Dispose(); $tile.Dispose() }

    $result = [Drawing.Bitmap]::new($Size, $Size, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $result.SetResolution(96, 96)
    $reducer = [Drawing.Graphics]::FromImage($result)
    $attributes = [Drawing.Imaging.ImageAttributes]::new()
    $attributes.SetWrapMode([Drawing.Drawing2D.WrapMode]::TileFlipXY)
    try {
        $reducer.Clear([Drawing.Color]::Transparent)
        $reducer.CompositingMode = [Drawing.Drawing2D.CompositingMode]::SourceCopy
        $reducer.CompositingQuality = [Drawing.Drawing2D.CompositingQuality]::HighQuality
        $reducer.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $reducer.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $reducer.DrawImage($large, [Drawing.Rectangle]::new(0, 0, $Size, $Size), 0, 0, $largeSize, $largeSize, [Drawing.GraphicsUnit]::Pixel, $attributes)
    } finally { $reducer.Dispose(); $attributes.Dispose(); $large.Dispose() }
    return $result
}

$sizes = @(16, 24, 32, 48, 64, 128, 256)
$frames = @()
foreach ($size in $sizes) {
    $bitmap = New-IconFrame $size
    $stream = [IO.MemoryStream]::new()
    try {
        $transparent = 0; $opaque = 0; $partial = 0
        for ($y = 0; $y -lt $size; $y++) {
            for ($x = 0; $x -lt $size; $x++) {
                $alpha = $bitmap.GetPixel($x, $y).A
                if ($alpha -eq 0) { $transparent++ } elseif ($alpha -eq 255) { $opaque++ } else { $partial++ }
            }
        }
        if ($bitmap.PixelFormat -ne [Drawing.Imaging.PixelFormat]::Format32bppArgb -or $transparent -eq 0 -or $opaque -eq 0 -or $partial -eq 0 -or $bitmap.GetPixel(0, 0).A -ne 0) { throw "Frame does not preserve transparent antialiased corners: $size" }
        $preview = Join-Path $PreviewDirectory ("icon-{0}.png" -f $size)
        $bitmap.Save($preview, [Drawing.Imaging.ImageFormat]::Png)
        $bitmap.Save($stream, [Drawing.Imaging.ImageFormat]::Png)
        $frames += @{ Size = $size; Bytes = $stream.ToArray(); Transparent = $transparent; Opaque = $opaque; Partial = $partial; Preview = $preview }
    } finally { $bitmap.Dispose(); $stream.Dispose() }
}

# ICO directory records are little-endian; 0 means 256 for width/height.
# PNG-compressed RGBA frames are supported by Windows Vista and newer.
$iconPath = Join-Path $iconDirectory 'icon.ico'
$output = [IO.File]::Create($iconPath)
$writer = [IO.BinaryWriter]::new($output)
try {
    $writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$frames.Count)
    $offset = 6 + 16 * $frames.Count
    foreach ($frame in $frames) {
        $dimension = if ($frame.Size -eq 256) { 0 } else { $frame.Size }
        $writer.Write([byte]$dimension); $writer.Write([byte]$dimension)
        $writer.Write([byte]0); $writer.Write([byte]0)
        $writer.Write([uint16]1); $writer.Write([uint16]32)
        $writer.Write([uint32]$frame.Bytes.Length); $writer.Write([uint32]$offset)
        $offset += $frame.Bytes.Length
    }
    foreach ($frame in $frames) { $writer.Write([byte[]]$frame.Bytes) }
} finally { $writer.Dispose(); $output.Dispose() }

# Independently parse the saved directory and inspect each embedded PNG.
$saved = [IO.File]::ReadAllBytes($iconPath)
if ([BitConverter]::ToUInt16($saved, 2) -ne 1 -or [BitConverter]::ToUInt16($saved, 4) -ne $sizes.Count) { throw 'ICO directory validation failed.' }
for ($index = 0; $index -lt $sizes.Count; $index++) {
    $entry = 6 + $index * 16
    $width = if ($saved[$entry] -eq 0) { 256 } else { $saved[$entry] }
    $height = if ($saved[$entry + 1] -eq 0) { 256 } else { $saved[$entry + 1] }
    $bits = [BitConverter]::ToUInt16($saved, $entry + 6)
    $length = [BitConverter]::ToUInt32($saved, $entry + 8)
    $offset = [BitConverter]::ToUInt32($saved, $entry + 12)
    $pngBytes = [byte[]]::new($length)
    [Array]::Copy($saved, $offset, $pngBytes, 0, $length)
    $input = [IO.MemoryStream]::new($pngBytes, $false)
    $image = [Drawing.Bitmap]::new($input)
    try {
        if ($width -ne $sizes[$index] -or $height -ne $sizes[$index] -or $bits -ne 32 -or $image.Width -ne $width -or $image.Height -ne $height -or $image.GetPixel(0, 0).A -ne 0 -or $pngBytes[24] -ne 8 -or $pngBytes[25] -ne 6) { throw "Saved RGBA ICO frame validation failed: $width" }
    } finally { $image.Dispose(); $input.Dispose() }
    [PSCustomObject]@{ Size = "${width}x${height}"; BitsPerPixel = $bits; Encoding = 'PNG RGBA'; TransparentPixels = $frames[$index].Transparent; PartialAlphaPixels = $frames[$index].Partial; Preview = $frames[$index].Preview }
}

# A separate preview sheet makes small native sizes easy to inspect on both
# light and dark backgrounds; it is intentionally outside the source tree.
$sheet = [Drawing.Bitmap]::new(720, 220, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
$sheetGraphics = [Drawing.Graphics]::FromImage($sheet)
$sheetGraphics.Clear([Drawing.Color]::White)
$darkBrush = [Drawing.SolidBrush]::new((Get-IconColor '#0C0D14'))
$labelBrush = [Drawing.SolidBrush]::new((Get-IconColor '#555865'))
$lightLabelBrush = [Drawing.SolidBrush]::new((Get-IconColor '#B8BAC8'))
$labelFont = [Drawing.Font]::new('Segoe UI', 12, [Drawing.FontStyle]::Regular, [Drawing.GraphicsUnit]::Pixel)
try {
    $sheetGraphics.FillRectangle($darkBrush, 0, 110, 720, 110)
    $position = 25
    foreach ($size in @(16, 24, 32, 48, 64)) {
        $native = [Drawing.Bitmap]::new((Join-Path $PreviewDirectory ("icon-{0}.png" -f $size)))
        try {
            $sheetGraphics.DrawImageUnscaled($native, $position, (22 + [int]((64 - $size) / 2)))
            $sheetGraphics.DrawImageUnscaled($native, $position, (132 + [int]((64 - $size) / 2)))
            $sheetGraphics.DrawString("${size}px", $labelFont, $labelBrush, [float]$position, [float]89)
            $sheetGraphics.DrawString("${size}px", $labelFont, $lightLabelBrush, [float]$position, [float]199)
        } finally { $native.Dispose() }
        $position += 86
    }
    $large = [Drawing.Bitmap]::new((Join-Path $PreviewDirectory 'icon-256.png'))
    try { $sheetGraphics.DrawImage($large, [Drawing.Rectangle]::new(510, 14, 192, 192)) } finally { $large.Dispose() }
    $sheet.Save((Join-Path $PreviewDirectory 'icon-contact-sheet.png'), [Drawing.Imaging.ImageFormat]::Png)
} finally {
    $sheetGraphics.Dispose(); $sheet.Dispose(); $darkBrush.Dispose()
    $labelBrush.Dispose(); $lightLabelBrush.Dispose(); $labelFont.Dispose()
}
Write-Output "Saved icon: $iconPath"

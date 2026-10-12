[CmdletBinding()]
param(
    [string]$PreviewDirectory = (Join-Path $PSScriptRoot '../../installer-art-preview')
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$projectDirectory = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$iconDirectory = Join-Path $projectDirectory 'src-tauri/icons'
$PreviewDirectory = [IO.Path]::GetFullPath($PreviewDirectory)
New-Item -ItemType Directory -Force -Path $iconDirectory, $PreviewDirectory | Out-Null

function Get-Color([string]$Hex, [int]$Alpha = 255) {
    $rgb = [Drawing.ColorTranslator]::FromHtml($Hex)
    return [Drawing.Color]::FromArgb($Alpha, $rgb.R, $rgb.G, $rgb.B)
}

function New-Canvas([int]$Width, [int]$Height) {
    $bitmap = [Drawing.Bitmap]::new($Width, $Height, [Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $bitmap.SetResolution(96, 96)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    $graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $graphics.TextRenderingHint = [Drawing.Text.TextRenderingHint]::AntiAliasGridFit
    $graphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    return @{ Bitmap = $bitmap; Graphics = $graphics; Width = $Width; Height = $Height }
}

function Draw-Polygon($Canvas, [double[][]]$Coordinates, [Drawing.Color]$Color) {
    $points = [Drawing.PointF[]]@($Coordinates | ForEach-Object { [Drawing.PointF]::new($_[0], $_[1]) })
    $brush = [Drawing.SolidBrush]::new($Color)
    try { $Canvas.Graphics.FillPolygon($brush, $points) } finally { $brush.Dispose() }
}

function Draw-Line($Canvas, [float]$X1, [float]$Y1, [float]$X2, [float]$Y2, [Drawing.Color]$Color, [float]$Width = 1) {
    $pen = [Drawing.Pen]::new($Color, $Width)
    try { $Canvas.Graphics.DrawLine($pen, $X1, $Y1, $X2, $Y2) } finally { $pen.Dispose() }
}

function Draw-Monogram($Canvas, [float]$X, [float]$Y, [float]$Width, [int]$Alpha = 255) {
    $scale = $Width / 64
    $v = @(@(0, 0), @(12, 0), @(22, 29), @(32, 0), @(44, 0), @(27, 48), @(17, 48))
    $p = @(@(42, 0), @(64, 0), @(64, 25), @(53, 25), @(53, 48), @(41, 48), @(41, 10))
    $hole = @(@(52, 7), @(59, 12), @(52, 18))
    foreach ($shape in @(@{ Points = $v; Color = Get-Color '#F4F5F8' $Alpha }, @{ Points = $p; Color = Get-Color '#FF6575' $Alpha })) {
        $mapped = [double[][]]@($shape.Points | ForEach-Object { ,@(($X + ($_[0] * $scale)), ($Y + ($_[1] * $scale))) })
        Draw-Polygon $Canvas $mapped $shape.Color
    }
    if ($Alpha -eq 255) {
        $mappedHole = [double[][]]@($hole | ForEach-Object { ,@(($X + ($_[0] * $scale)), ($Y + ($_[1] * $scale))) })
        Draw-Polygon $Canvas $mappedHole (Get-Color '#15161F')
    }
}

function Draw-Text($Canvas, [string]$Text, [float]$X, [float]$Y, [float]$Size, [Drawing.Color]$Color, [bool]$Bold = $false, [float]$MaxWidth = 0) {
    $style = if ($Bold) { [Drawing.FontStyle]::Bold } else { [Drawing.FontStyle]::Regular }
    $font = [Drawing.Font]::new('Segoe UI', $Size, $style, [Drawing.GraphicsUnit]::Pixel)
    $format = [Drawing.StringFormat]::new([Drawing.StringFormat]::GenericTypographic)
    $format.FormatFlags = [Drawing.StringFormatFlags]::NoWrap -bor [Drawing.StringFormatFlags]::MeasureTrailingSpaces
    $brush = [Drawing.SolidBrush]::new($Color)
    try {
        $measured = $Canvas.Graphics.MeasureString($Text, $font, [Drawing.PointF]::new(0, 0), $format)
        if ($MaxWidth -gt 0 -and $measured.Width -gt $MaxWidth) { throw "Text exceeds its design width: $Text" }
        if ($X -lt 0 -or $Y -lt 0 -or $X + $measured.Width -gt $Canvas.Width -or $Y + $measured.Height -gt $Canvas.Height) { throw "Text would be clipped: $Text" }
        $Canvas.Graphics.DrawString($Text, $font, $brush, [Drawing.PointF]::new($X, $Y), $format)
    } finally { $font.Dispose(); $format.Dispose(); $brush.Dispose() }
}

function Draw-Background($Canvas) {
    $bounds = [Drawing.Rectangle]::new(0, 0, $Canvas.Width, $Canvas.Height)
    $gradient = [Drawing.Drawing2D.LinearGradientBrush]::new($bounds, (Get-Color '#191A25'), (Get-Color '#0C0D13'), [float]115)
    try { $Canvas.Graphics.FillRectangle($gradient, $bounds) } finally { $gradient.Dispose() }
}

function Save-Art($Canvas, [string]$Name) {
    $bitmapPath = Join-Path $iconDirectory ($Name + '.bmp')
    $previewPath = Join-Path $PreviewDirectory ($Name + '.png')
    $Canvas.Bitmap.Save($bitmapPath, [Drawing.Imaging.ImageFormat]::Bmp)
    $Canvas.Bitmap.Save($previewPath, [Drawing.Imaging.ImageFormat]::Png)
    $bytes = [IO.File]::ReadAllBytes($bitmapPath)
    $width = [BitConverter]::ToInt32($bytes, 18)
    $height = [Math]::Abs([BitConverter]::ToInt32($bytes, 22))
    $bits = [BitConverter]::ToUInt16($bytes, 28)
    $compression = [BitConverter]::ToUInt32($bytes, 30)
    if ($bytes[0] -ne 0x42 -or $bytes[1] -ne 0x4D -or $bits -ne 24 -or $compression -ne 0 -or $width -ne $Canvas.Width -or $height -ne $Canvas.Height) { throw "NSIS BMP validation failed: $bitmapPath" }
    [PSCustomObject]@{ File = $bitmapPath; Width = $width; Height = $height; BitsPerPixel = $bits; Compression = 'BI_RGB'; Preview = $previewPath }
}

$sidebar = New-Canvas 164 314
$header = New-Canvas 150 57
try {
    Draw-Background $sidebar
    Draw-Polygon $sidebar @(@(108, 0), @(164, 0), @(164, 172), @(95, 103)) (Get-Color '#FF6575' 9)
    Draw-Line $sidebar 108 0 164 56 (Get-Color '#FF6575' 50)
    Draw-Line $sidebar 125 0 164 39 (Get-Color '#F5F6FA' 15)
    Draw-Line $sidebar 20 26 43 26 (Get-Color '#FF6575') 2
    Draw-Text $sidebar 'WINDOWS' 54 21 8 (Get-Color '#8F909F') $true 90
    Draw-Monogram $sidebar 20 61 64
    Draw-Text $sidebar 'ValoPlayer' 19 128 22 (Get-Color '#F4F5F8') $true 128
    Draw-Text $sidebar 'MOMENTS, KEPT.' 20 162 9 (Get-Color '#A2A4B3') $true 124
    Draw-Line $sidebar 20 191 144 191 (Get-Color '#F4F5F8' 20)
    Draw-Polygon $sidebar @(@(164, 199), @(164, 314), @(88, 314), @(143, 253), @(117, 228)) (Get-Color '#FF6575' 12)
    Draw-Line $sidebar 88 314 164 230 (Get-Color '#FF6575' 35)
    Draw-Line $sidebar 109 314 164 253 (Get-Color '#F4F5F8' 14)
    Draw-Text $sidebar 'YOUR MOMENTS.' 20 246 10 (Get-Color '#F4F5F8') $true 124
    Draw-Text $sidebar 'YOUR LIBRARY.' 20 261 10 (Get-Color '#F4F5F8') $true 124
    Draw-Line $sidebar 20 289 144 289 (Get-Color '#FF6575') 1

    Draw-Background $header
    Draw-Polygon $header @(@(124, 0), @(150, 0), @(150, 57), @(109, 16)) (Get-Color '#FF6575' 12)
    Draw-Monogram $header 11 17 28
    Draw-Text $header 'ValoPlayer' 48 13 14 (Get-Color '#F4F5F8') $true 96
    Draw-Text $header 'MOMENTS, KEPT.' 49 34 7 (Get-Color '#A2A4B3') $true 95
    Draw-Line $header 0 56 150 56 (Get-Color '#FF6575' 135)

    Save-Art $sidebar 'installer-sidebar'
    Save-Art $header 'installer-header'
} finally {
    $sidebar.Graphics.Dispose(); $sidebar.Bitmap.Dispose()
    $header.Graphics.Dispose(); $header.Bitmap.Dispose()
}

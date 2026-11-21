<#
.SYNOPSIS
  Generates all PILLVYN application, tray, UI, documentation, and Windows ICO icons
  from the master branding PNG (`docs/branding/logo v2.png` or `docs/branding/pillvyn-logo.png`).
#>
param(
  [string]$Source = ''
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
if (-not $Source) {
  $v2 = Join-Path $root 'docs\branding\logo v2.png'
  $canonical = Join-Path $root 'docs\branding\pillvyn-logo.png'
  $Source = if (Test-Path $v2) { $v2 } else { $canonical }
}
if (-not (Test-Path $Source)) {
  throw "Logo source not found: $Source"
}

$srcBmp = [System.Drawing.Bitmap]::new($Source)
try {
  # Find tight non-transparent bounding box so small tray/taskbar icons have crisp optical scale
  $minX = $srcBmp.Width; $minY = $srcBmp.Height; $maxX = 0; $maxY = 0
  for ($y = 0; $y -lt $srcBmp.Height; $y++) {
    for ($x = 0; $x -lt $srcBmp.Width; $x++) {
      if ($srcBmp.GetPixel($x, $y).A -gt 10) {
        if ($x -lt $minX) { $minX = $x }
        if ($x -gt $maxX) { $maxX = $x }
        if ($y -lt $minY) { $minY = $y }
        if ($y -gt $maxY) { $maxY = $y }
      }
    }
  }
  $cw = $maxX - $minX + 1
  $ch = $maxY - $minY + 1
  $side = [Math]::Max($cw, $ch)
  $cx = ($minX + $maxX) / 2.0
  $cy = ($minY + $maxY) / 2.0

  function Render-ResizedPng([int]$px, [double]$padRatio) {
    $bmp = [System.Drawing.Bitmap]::new($px, $px, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    try {
      $g.Clear([System.Drawing.Color]::Transparent)
      $g.CompositingMode = [System.Drawing.Drawing2D.CompositingMode]::SourceCopy
      $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
      $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
      $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
      $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality

      $pad = [float]($px * $padRatio)
      $dstSize = [float]($px - 2.0 * $pad)
      $srcHalf = [float]($side / 2.0)
      $srcRect = [System.Drawing.RectangleF]::new([float]($cx - $srcHalf), [float]($cy - $srcHalf), [float]$side, [float]$side)
      $dstRect = [System.Drawing.RectangleF]::new($pad, $pad, $dstSize, $dstSize)
      $g.DrawImage($srcBmp, $dstRect, $srcRect, [System.Drawing.GraphicsUnit]::Pixel)
    } finally {
      $g.Dispose()
    }
    $ms = [System.IO.MemoryStream]::new()
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    return $ms.ToArray()
  }

  $iconsDir = Join-Path $root 'src-tauri\icons'
  New-Item -ItemType Directory -Force $iconsDir | Out-Null

  $png32  = Render-ResizedPng 32  0.03
  $png128 = Render-ResizedPng 128 0.04
  $png256 = Render-ResizedPng 256 0.04
  $png512 = Render-ResizedPng 512 0.04

  [System.IO.File]::WriteAllBytes((Join-Path $iconsDir '32x32.png'),      $png32)
  [System.IO.File]::WriteAllBytes((Join-Path $iconsDir '128x128.png'),    $png128)
  [System.IO.File]::WriteAllBytes((Join-Path $iconsDir '128x128@2x.png'), $png256)
  [System.IO.File]::WriteAllBytes((Join-Path $iconsDir 'icon.png'),       $png512)
  [System.IO.File]::WriteAllBytes((Join-Path $root 'docs\icon.png'),      $png256)
  [System.IO.File]::WriteAllBytes((Join-Path $root 'ui\icon.png'),        $png256)
  [System.IO.File]::WriteAllBytes((Join-Path $root 'docs\branding\pillvyn-logo.png'), $png512)

  # Build multi-resolution Windows ICO with embedded PNG frames (16, 24, 32, 48, 64, 128, 256)
  $icoSizes = @(16, 24, 32, 48, 64, 128, 256)
  $frames = @()
  foreach ($s in $icoSizes) {
    $pad = if ($s -le 32) { 0.02 } else { 0.04 }
    $frames += ,@($s, (Render-ResizedPng $s $pad))
  }

  $icoMs = [System.IO.MemoryStream]::new()
  $bw = [System.IO.BinaryWriter]::new($icoMs)
  try {
    $bw.Write([uint16]0)              # Reserved
    $bw.Write([uint16]1)              # Type: ICO
    $bw.Write([uint16]$frames.Count)  # Image count
    $offset = 6 + (16 * $frames.Count)
    foreach ($f in $frames) {
      $sz = [int]$f[0]
      $data = [byte[]]$f[1]
      $dimByte = if ($sz -ge 256) { [byte]0 } else { [byte]$sz }
      $bw.Write($dimByte)
      $bw.Write($dimByte)
      $bw.Write([byte]0)
      $bw.Write([byte]0)
      $bw.Write([uint16]1)
      $bw.Write([uint16]32)
      $bw.Write([uint32]$data.Length)
      $bw.Write([uint32]$offset)
      $offset += $data.Length
    }
    foreach ($f in $frames) {
      $bw.Write([byte[]]$f[1])
    }
    $bw.Flush()
    [System.IO.File]::WriteAllBytes((Join-Path $iconsDir 'icon.ico'), $icoMs.ToArray())
  } finally {
    $bw.Dispose()
    $icoMs.Dispose()
  }

  Write-Host "Generated all PILLVYN icons cleanly from: $Source"
  Get-Item @(
    "$iconsDir\32x32.png",
    "$iconsDir\128x128.png",
    "$iconsDir\128x128@2x.png",
    "$iconsDir\icon.png",
    "$iconsDir\icon.ico",
    "$root\docs\icon.png",
    "$root\ui\icon.png",
    "$root\docs\branding\pillvyn-logo.png"
  ) | Select-Object Name, Length | Format-Table -AutoSize
} finally {
  $srcBmp.Dispose()
}

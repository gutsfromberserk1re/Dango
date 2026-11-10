Add-Type -AssemblyName System.Drawing

$artifactDir = Join-Path $PSScriptRoot '..\src-tauri\windows'
New-Item -ItemType Directory -Force -Path $artifactDir | Out-Null
$cream = [System.Drawing.Color]::FromArgb(248, 241, 226)
$ink = [System.Drawing.Color]::FromArgb(85, 66, 78)
$font = [System.Drawing.Font]::new('Segoe UI', 22, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
$smallFont = [System.Drawing.Font]::new('Segoe UI', 9, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
$brush = [System.Drawing.SolidBrush]::new($ink)
$background = [System.Drawing.SolidBrush]::new($cream)
$format = [System.Drawing.StringFormat]::new()
$format.Alignment = [System.Drawing.StringAlignment]::Center
$format.LineAlignment = [System.Drawing.StringAlignment]::Center

try {
  $dialog = [System.Drawing.Bitmap]::new(493, 312)
  $graphics = [System.Drawing.Graphics]::FromImage($dialog)
  try {
    $graphics.Clear($cream)
    $icon = [System.Drawing.Icon]::new((Join-Path $PSScriptRoot '..\icons\Dango.ico'))
    try { $graphics.DrawIcon($icon, [System.Drawing.Rectangle]::new(204, 84, 84, 84)) } finally { $icon.Dispose() }
    $graphics.DrawString('Dango', $font, $brush, [System.Drawing.RectangleF]::new(0, 176, 493, 40), $format)
    $graphics.DrawString('Video editor', $smallFont, $brush, [System.Drawing.RectangleF]::new(0, 215, 493, 20), $format)
  } finally { $graphics.Dispose() }
  $dialog.Save((Join-Path $artifactDir 'installer-dialog.bmp'), [System.Drawing.Imaging.ImageFormat]::Bmp)
  $dialog.Dispose()

  $banner = [System.Drawing.Bitmap]::new(493, 58)
  $graphics = [System.Drawing.Graphics]::FromImage($banner)
  try {
    $graphics.Clear($cream)
    $icon = [System.Drawing.Icon]::new((Join-Path $PSScriptRoot '..\icons\Dango.ico'))
    try { $graphics.DrawIcon($icon, [System.Drawing.Rectangle]::new(12, 8, 42, 42)) } finally { $icon.Dispose() }
    $graphics.DrawString('Dango', [System.Drawing.Font]::new('Segoe UI', 13, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel), $brush, [System.Drawing.PointF]::new(64, 18))
  } finally { $graphics.Dispose() }
  $banner.Save((Join-Path $artifactDir 'installer-banner.bmp'), [System.Drawing.Imaging.ImageFormat]::Bmp)
  $banner.Dispose()
} finally {
  $font.Dispose()
  $smallFont.Dispose()
  $brush.Dispose()
  $background.Dispose()
  $format.Dispose()
}

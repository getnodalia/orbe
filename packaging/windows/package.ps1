<#
.SYNOPSIS
  Build, sign and package the Orbe editor for Windows.

.DESCRIPTION
  Produces, in $env:DIST (default: dist):
    orbe-<version>-windows-<arch>.msi            per-user installer with a setup wizard (WiX v5)
    orbe-<version>-windows-<arch>-portable.zip   orbe.exe + orbe.console.exe + a `_sc_` file

  The engine links the C runtime statically (use_static_cpp), so neither the MSI nor the portable
  zip needs the Visual C++ redistributable. The build needs no Vulkan SDK, no Direct3D 12 SDK and
  no prebuilt ANGLE (d3d12=no angle=no): Vulkan is loaded at run time and OpenGL is native, so
  nothing here comes from outside this repository. Signing is delegated to sign.ps1 (skipped with
  a warning when no signing secrets are set).

  Needs: Python and SCons 4.4 or newer, the MSVC toolchain for the target (the ARM64 and x86 tools
  for -Arch arm64 and x86) with the Windows SDK, and WiX v5:
    dotnet tool install --global wix --version 5.0.2
    wix extension add -g WixToolset.UI.wixext/5.0.2 WixToolset.Util.wixext/5.0.2

.EXAMPLE
  pwsh packaging/windows/package.ps1 -Arch x64
  pwsh packaging/windows/package.ps1 -Arch x86 -SkipBuild
  pwsh packaging/windows/package.ps1 -Arch arm64     # cross-compiled; needs the MSVC ARM64 build tools
#>
param(
  [ValidateSet('x64', 'x86', 'arm64')] [string] $Arch = 'x64',
  [switch] $SkipBuild
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

function Invoke-Native([string] $What, [scriptblock] $Block) {
  Write-Output "==> $What"
  & $Block
  if ($LASTEXITCODE -ne 0) { throw "$What failed with exit code $LASTEXITCODE" }
}

# The version lives in one place: ORBE_VERSION.
$Version = $env:ORBE_PACKAGE_VERSION
if (-not $Version) { $Version = (Get-Content -Raw (Join-Path $Root 'ORBE_VERSION')).Trim() }
if (-not $Version) { throw 'ORBE_VERSION is empty' }
# MSI ProductVersion is numeric (major.minor.build); pre-release tags are dropped there.
$MsiVersion = ($Version -split '-')[0]

# The engine's own architecture names, and the PE machine type each must produce.
$SconsArch = switch ($Arch) { 'x64' { 'x86_64' } 'x86' { 'x86_32' } 'arm64' { 'arm64' } }
$Machine = switch ($Arch) { 'x64' { 0x8664 } 'x86' { 0x14C } 'arm64' { 0xAA64 } }
$Dist = if ($env:DIST) { $env:DIST } else { Join-Path $Root 'dist' }
New-Item -ItemType Directory -Force -Path $Dist | Out-Null
$Stage = Join-Path $Root "bin\windows-package\$Arch"

if (-not $env:ORBE_BUILD_DATE) { $env:ORBE_BUILD_DATE = (Get-Date).ToUniversalTime().ToString('yyyy-MM-dd') }

Write-Output "Orbe $Version for Windows $Arch"

$Exe = Join-Path $Root "bin\orbe.windows.editor.$SconsArch.exe"
$ConsoleExe = Join-Path $Root "bin\orbe.windows.editor.$SconsArch.console.exe"
if (-not $SkipBuild) {
  Push-Location $Root
  try {
    Invoke-Native "scons (windows $SconsArch)" {
      scons platform=windows target=editor arch=$SconsArch d3d12=no angle=no accesskit=no `
        module_mono_enabled=no optimize=speed lto=none debug_symbols=no "-j$env:NUMBER_OF_PROCESSORS"
    }
  } finally { Pop-Location }
}
foreach ($f in $Exe, $ConsoleExe) { if (-not (Test-Path $f)) { throw "$f is missing" } }

# Check both binaries' PE headers before packaging. Machine (COFF header) must match -Arch, so an
# x64 build can never ship labelled arm64. Subsystem (optional header): 2 = Windows GUI, 3 = console.
# The editor must be GUI (no console window opens with it); the console build must stay console so
# its output reaches the terminal.
function Get-PeHeader([string] $Path) {
  $bytes = [System.IO.File]::ReadAllBytes($Path)
  $pe = [BitConverter]::ToInt32($bytes, 0x3C)
  return @{ Machine = [BitConverter]::ToUInt16($bytes, $pe + 4); Subsystem = [BitConverter]::ToUInt16($bytes, $pe + 0x5C) }
}
foreach ($check in @(@($Exe, 2), @($ConsoleExe, 3))) {
  $name = Split-Path -Leaf $check[0]
  $h = Get-PeHeader $check[0]
  if ($h.Machine -ne $Machine) { throw "$name is for machine 0x$('{0:X}' -f $h.Machine), expected 0x$('{0:X}' -f $Machine) ($Arch)" }
  if ($h.Subsystem -ne $check[1]) { throw "$name has PE subsystem $($h.Subsystem), expected $($check[1])" }
  Write-Output "ok ${name}: $Arch, PE subsystem $($h.Subsystem)"
}

Remove-Item -Recurse -Force $Stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Stage | Out-Null
Copy-Item $Exe (Join-Path $Stage 'orbe.exe')
# The console build finds the editor by its own name minus ".console", so the pair keeps one stem.
Copy-Item $ConsoleExe (Join-Path $Stage 'orbe.console.exe')
foreach ($f in 'LICENSE.txt', 'COPYRIGHT.txt', 'NOTICE') { Copy-Item (Join-Path $Root $f) $Stage }

& (Join-Path $PSScriptRoot 'sign.ps1') (Join-Path $Stage 'orbe.exe') (Join-Path $Stage 'orbe.console.exe')

# ---- MSI ---------------------------------------------------------------------------------------
# The setup wizard's art, drawn here (System.Drawing) in Orbe's colours so the MSI shows Orbe
# rather than WiX's stock bitmaps. Both must be 24-bit BMPs:
#   - dialog, 493x312: the Welcome and Finish pages' backdrop. Their text sits on the right, so
#     that stays white; the left panel is ink with the mark and the name.
#   - banner, 493x58: across the top of the inner pages. The page title is drawn over its left
#     side, so that stays white; the right end is a dark cell with the mark.
function New-InstallerArt([string] $BannerOut, [string] $DialogOut) {
  Add-Type -AssemblyName System.Drawing
  $ink = [System.Drawing.ColorTranslator]::FromHtml('#121316')
  $accent = [System.Drawing.ColorTranslator]::FromHtml('#DDA10A')
  $mark = [System.Drawing.Image]::FromFile((Join-Path $Root 'misc\logo\icon.png'))
  $nameFont = New-Object System.Drawing.Font 'Segoe UI Semibold', 26, ([System.Drawing.FontStyle]::Regular), ([System.Drawing.GraphicsUnit]::Pixel)
  try {
    foreach ($spec in @(@($BannerOut, 493, 58), @($DialogOut, 493, 312))) {
      $bmp = New-Object System.Drawing.Bitmap $spec[1], $spec[2], ([System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
      $g = [System.Drawing.Graphics]::FromImage($bmp)
      $panel = New-Object System.Drawing.SolidBrush $ink
      $stripe = New-Object System.Drawing.SolidBrush $accent
      try {
        $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
        $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
        $g.Clear([System.Drawing.Color]::White)
        if ($spec[2] -eq 58) {
          $g.FillRectangle($panel, 405, 0, 88, 58)
          $g.FillRectangle($stripe, 405, 0, 2, 58)
          $g.DrawImage($mark, 431, 11, 36, 36)
        } else {
          $g.FillRectangle($panel, 0, 0, 164, 312)
          $g.FillRectangle($stripe, 162, 0, 2, 312)
          $g.DrawImage($mark, 26, 60, 112, 112)
          $white = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 242, 241, 238))
          $g.DrawString('Orbe', $nameFont, $white, 32, 186, [System.Drawing.StringFormat]::GenericTypographic)
          $white.Dispose()
        }
      } finally { $panel.Dispose(); $stripe.Dispose(); $g.Dispose() }
      $bmp.Save($spec[0], [System.Drawing.Imaging.ImageFormat]::Bmp)
      $bmp.Dispose()
    }
  } finally { $nameFont.Dispose(); $mark.Dispose() }
}
$Banner = Join-Path $Stage 'installer-banner.bmp'
$Dialog = Join-Path $Stage 'installer-dialog.bmp'
New-InstallerArt $Banner $Dialog

$Msi = Join-Path $Dist "orbe-$Version-windows-$Arch.msi"
Invoke-Native 'wix build' {
  wix build (Join-Path $PSScriptRoot 'orbe.wxs') -arch $Arch `
    -ext WixToolset.UI.wixext -ext WixToolset.Util.wixext `
    -culture en-US -loc (Join-Path $PSScriptRoot 'orbe.en-us.wxl') `
    -d "Version=$MsiVersion" -d "DisplayVersion=$Version" -d "BinDir=$Stage" `
    -d "IconPath=$(Join-Path $Root 'platform\windows\godot.ico')" `
    -d "BannerBmp=$Banner" -d "DialogBmp=$Dialog" `
    -o $Msi
}
Invoke-Native 'MSI shortcut icon validation (ICE50)' {
  wix msi validate $Msi -ice ICE50 -intermediateFolder (Join-Path $Stage 'msi-validation')
}
# wix writes its debug symbols (.wixpdb) next to the MSI; keep them out of the release assets.
Remove-Item -Force -ErrorAction SilentlyContinue ([IO.Path]::ChangeExtension($Msi, '.wixpdb'))
& (Join-Path $PSScriptRoot 'sign.ps1') $Msi

# ---- portable zip ------------------------------------------------------------------------------
$Portable = Join-Path $Root "bin\windows-package\orbe-$Version-windows-$Arch-portable"
Remove-Item -Recurse -Force $Portable -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Portable | Out-Null
Copy-Item (Join-Path $Stage '*.exe') $Portable
foreach ($f in 'README.md', 'LICENSE.txt', 'COPYRIGHT.txt', 'NOTICE', 'ATTRIBUTION.md') {
  $p = Join-Path $Root $f
  if (Test-Path $p) { Copy-Item $p $Portable }
}
# A file called `_sc_` beside the editor keeps its settings, cache and templates in this folder
# instead of %APPDATA%: that is what makes the zip portable.
New-Item -ItemType File -Force -Path (Join-Path $Portable '_sc_') | Out-Null
$Zip = Join-Path $Dist "orbe-$Version-windows-$Arch-portable.zip"
Remove-Item -Force $Zip -ErrorAction SilentlyContinue
Compress-Archive -Path $Portable -DestinationPath $Zip

# Smoke-test the console build when this machine can run it. An ARM64 or x86 build made on an
# x64 runner may not run here.
$HostArch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
if ($Arch -eq $HostArch -or ($Arch -eq 'x86' -and $HostArch -eq 'x64')) {
  Invoke-Native 'orbe.console --headless --version' { & (Join-Path $Stage 'orbe.console.exe') --headless --version }
} else {
  Write-Output "skipping the version check: a $Arch build doesn't run on this $HostArch machine"
}
Get-Item $Msi, $Zip | Format-Table Name, Length

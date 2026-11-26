<#
.SYNOPSIS
    Builds the Windows Inno Setup installer for Kraken VPN.
.PARAMETER SkipBuild
    Skip `cargo build --release` and package the existing release binary.
#>
param(
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot

try {
    # 1. Resolve version from Cargo.toml
    $match = Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
    if (-not $match) { throw "Could not find package version in Cargo.toml" }
    $version = $match.Matches[0].Groups[1].Value
    Write-Host "Packaging Kraken VPN v$version installer..."

    # 2. Build release executable if requested
    if (-not $SkipBuild) {
        cargo build --release --locked
        if ($LASTEXITCODE -ne 0) { throw "cargo build --release failed" }
    }
    if (-not (Test-Path ".\target\release\kraken-vpn.exe")) {
        throw "target\release\kraken-vpn.exe not found. Run cargo build --release first."
    }

    # 3. Convert icon.png -> installer\kraken-vpn.ico (256x256 PNG-in-ICO)
    if (Test-Path "icon.png") {
        Add-Type -AssemblyName System.Drawing
        $src = [System.Drawing.Image]::FromFile((Resolve-Path "icon.png"))
        $bmp = New-Object System.Drawing.Bitmap 256, 256
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $g.DrawImage($src, 0, 0, 256, 256)
        $g.Dispose()
        $src.Dispose()

        $pngStream = New-Object System.IO.MemoryStream
        $bmp.Save($pngStream, [System.Drawing.Imaging.ImageFormat]::Png)
        $pngBytes = $pngStream.ToArray()
        $pngStream.Dispose()
        $bmp.Dispose()

        $icoPath = Join-Path $PSScriptRoot "kraken-vpn.ico"
        $fs = [System.IO.File]::Create($icoPath)
        $bw = New-Object System.IO.BinaryWriter($fs)
        # ICONDIR header (6 bytes)
        $bw.Write([UInt16]0)   # reserved
        $bw.Write([UInt16]1)   # type: icon
        $bw.Write([UInt16]1)   # count: 1 image
        # ICONDIRENTRY (16 bytes)
        $bw.Write([Byte]0)     # width: 0 means 256
        $bw.Write([Byte]0)     # height: 0 means 256
        $bw.Write([Byte]0)     # color count
        $bw.Write([Byte]0)     # reserved
        $bw.Write([UInt16]1)   # planes
        $bw.Write([UInt16]32)  # bpp
        $bw.Write([UInt32]$pngBytes.Length)
        $bw.Write([UInt32]22)  # offset = 6 + 16
        $bw.Write($pngBytes)
        $bw.Close()
        $fs.Close()
        Write-Host "Generated $icoPath"
    }

    # 4. Locate Inno Setup 6 compiler (ISCC.exe)
    $iscc = Get-Command iscc -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
    if (-not $iscc) {
        $candidates = @(
            "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
            "C:\Program Files\Inno Setup 6\ISCC.exe"
        )
        $iscc = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    }
    if (-not $iscc) {
        throw "Inno Setup 6 (ISCC.exe) not found. Install it via: winget install JRSoftware.InnoSetup"
    }

    # 5. Compile installer
    New-Item -ItemType Directory -Force -Path "release-artifacts" | Out-Null
    & $iscc "/DMyAppVersion=$version" (Join-Path $PSScriptRoot "kraken-vpn.iss")
    if ($LASTEXITCODE -ne 0) { throw "ISCC failed with exit code $LASTEXITCODE" }

    $setupExe = "release-artifacts\KrakenVPN-Setup-$version-windows-x86_64.exe"
    Write-Host "Built installer: $setupExe"
}
finally {
    Pop-Location
}

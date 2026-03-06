param(
    [Parameter(Mandatory = $true)]
    [string]$Version,
    [string]$BinaryPath = "target/release/aura.exe",
    [string]$OutputDir = "dist/squirrel",
    [string]$NuGetExe = "nuget",
    [string]$SquirrelExe = "",
    [string]$SquirrelWindowsVersion = "2.0.1"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-RepoRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
}

function Resolve-NuGetCommand {
    param([string]$CommandName)
    $nuget = Get-Command $CommandName -ErrorAction SilentlyContinue
    if (-not $nuget) {
        throw "NuGet command '$CommandName' was not found. Install NuGet.CommandLine first."
    }
    return $nuget.Source
}

function Resolve-SquirrelExecutable {
    param(
        [string]$ProvidedPath,
        [string]$NuGetPath,
        [string]$ToolsDir,
        [string]$PackageVersion
    )

    if ($ProvidedPath) {
        if (-not (Test-Path -LiteralPath $ProvidedPath)) {
            throw "Provided Squirrel executable does not exist: $ProvidedPath"
        }
        return (Resolve-Path -LiteralPath $ProvidedPath).Path
    }

    if (-not $PackageVersion) {
        throw "Squirrel.Windows package version must be provided."
    }

    New-Item -ItemType Directory -Path $ToolsDir -Force | Out-Null
    
    # Suppress the standard output to prevent pipeline pollution
    & $NuGetPath install Squirrel.Windows -Version $PackageVersion -OutputDirectory $ToolsDir -ExcludeVersion -NonInteractive | Out-Null
    
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to install Squirrel.Windows $PackageVersion via NuGet."
    }

    $candidate = Join-Path $ToolsDir "Squirrel.Windows\tools\Squirrel.exe"
    if (-not (Test-Path -LiteralPath $candidate)) {
        throw "Unable to locate Squirrel.exe after installing Squirrel.Windows $PackageVersion package."
    }

    return $candidate
}

function Convert-PngToIco {
    param(
        [string]$SourcePath,
        [string]$DestinationPath,
        [int]$Size = 256
    )

    if (-not (Test-Path -LiteralPath $SourcePath)) {
        throw "Cannot convert missing PNG to ICO: $SourcePath"
    }

    if ($Size -le 0) {
        throw "ICO size must be a positive integer. Received: $Size"
    }

    Add-Type -AssemblyName System.Drawing

    $targetSize = [Math]::Min($Size, 256)
    if ($targetSize -lt 256) {
        throw "ICO max size must be at least 256 for Squirrel setup icon compatibility. Received: $Size"
    }

    $iconSizes = @(16, 20, 24, 32, 48, 64, 256)
    $sourceImage = [System.Drawing.Image]::FromFile($SourcePath)
    try {
        $iconEntries = New-Object System.Collections.Generic.List[object]

        foreach ($iconSize in $iconSizes) {
            if ($iconSize -gt $targetSize) {
                continue
            }

            $bitmap = New-Object System.Drawing.Bitmap(
                $iconSize,
                $iconSize,
                [System.Drawing.Imaging.PixelFormat]::Format32bppArgb
            )
            try {
                $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
                try {
                    $graphics.Clear([System.Drawing.Color]::Transparent)
                    $graphics.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
                    $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
                    $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
                    $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
                    $graphics.DrawImage($sourceImage, 0, 0, $iconSize, $iconSize)
                }
                finally {
                    $graphics.Dispose()
                }

                $pngStream = New-Object System.IO.MemoryStream
                try {
                    $bitmap.Save($pngStream, [System.Drawing.Imaging.ImageFormat]::Png)
                    $pngBytes = $pngStream.ToArray()
                    $iconEntries.Add([PSCustomObject]@{
                            Size  = $iconSize
                            Bytes = $pngBytes
                        }) | Out-Null
                }
                finally {
                    $pngStream.Dispose()
                }
            }
            finally {
                $bitmap.Dispose()
            }
        }

        if ($iconEntries.Count -lt 1) {
            throw "Failed to generate any ICO entries from source PNG: $SourcePath"
        }

        $destinationDirectory = Split-Path -Path $DestinationPath -Parent
        if ($destinationDirectory) {
            New-Item -ItemType Directory -Path $destinationDirectory -Force | Out-Null
        }

        $icoStream = [System.IO.File]::Open(
            $DestinationPath,
            [System.IO.FileMode]::Create,
            [System.IO.FileAccess]::Write,
            [System.IO.FileShare]::None
        )
        try {
            $writer = New-Object System.IO.BinaryWriter($icoStream)
            try {
                $entryCount = $iconEntries.Count
                $imageOffset = 6 + (16 * $entryCount)

                # ICONDIR
                $writer.Write([UInt16]0)
                $writer.Write([UInt16]1)
                $writer.Write([UInt16]$entryCount)

                # ICONDIRENTRY table
                foreach ($entry in $iconEntries) {
                    $entrySize = [int]$entry.Size
                    $entryBytes = [byte[]]$entry.Bytes
                    $entryWidth = if ($entrySize -ge 256) { [byte]0 } else { [byte]$entrySize }
                    $entryHeight = if ($entrySize -ge 256) { [byte]0 } else { [byte]$entrySize }

                    $writer.Write($entryWidth)
                    $writer.Write($entryHeight)
                    $writer.Write([byte]0)
                    $writer.Write([byte]0)
                    $writer.Write([UInt16]0)
                    $writer.Write([UInt16]32)
                    $writer.Write([UInt32]$entryBytes.Length)
                    $writer.Write([UInt32]$imageOffset)

                    $imageOffset += $entryBytes.Length
                }

                # Image payloads
                foreach ($entry in $iconEntries) {
                    $writer.Write([byte[]]$entry.Bytes)
                }
            }
            finally {
                $writer.Dispose()
            }
        }
        finally {
            $icoStream.Dispose()
        }
    }
    finally {
        $sourceImage.Dispose()
    }
}

function Get-IcoEntries {
    param(
        [string]$Path
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        throw "Cannot read missing ICO file: $Path"
    }

    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 6) {
        throw "ICO file is too small to contain a valid header: $Path"
    }

    $reserved = [System.BitConverter]::ToUInt16($bytes, 0)
    $iconType = [System.BitConverter]::ToUInt16($bytes, 2)
    $entryCount = [System.BitConverter]::ToUInt16($bytes, 4)

    if ($reserved -ne 0 -or $iconType -ne 1) {
        throw "Invalid ICO header in '$Path' (reserved=$reserved, type=$iconType)"
    }

    $directorySize = 6 + (16 * $entryCount)
    if ($bytes.Length -lt $directorySize) {
        throw "ICO directory table is truncated in '$Path'"
    }

    $entries = New-Object System.Collections.Generic.List[object]
    for ($index = 0; $index -lt $entryCount; $index++) {
        $baseOffset = 6 + (16 * $index)
        $rawWidth = [int]$bytes[$baseOffset]
        $rawHeight = [int]$bytes[$baseOffset + 1]
        $reserved = [int]$bytes[$baseOffset + 3]
        $width = if ($rawWidth -eq 0) { 256 } else { $rawWidth }
        $height = if ($rawHeight -eq 0) { 256 } else { $rawHeight }
        $planes = [System.BitConverter]::ToUInt16($bytes, $baseOffset + 4)
        $bitCount = [System.BitConverter]::ToUInt16($bytes, $baseOffset + 6)
        $bytesInRes = [System.BitConverter]::ToUInt32($bytes, $baseOffset + 8)
        $imageOffset = [System.BitConverter]::ToUInt32($bytes, $baseOffset + 12)

        if (($imageOffset + $bytesInRes) -gt $bytes.Length) {
            throw "ICO image entry #$index points past EOF in '$Path'"
        }

        $entries.Add([PSCustomObject]@{
                Width      = $width
                Height     = $height
                Reserved   = $reserved
                Planes     = $planes
                BitCount   = $bitCount
                BytesInRes = $bytesInRes
                Offset     = $imageOffset
            }) | Out-Null
    }

    return $entries
}

function Assert-IcoHasRequiredSizes {
    param(
        [string]$Path,
        [int[]]$RequiredSizes = @(16, 32, 48, 256),
        [int]$MinEntryCount = 6
    )

    $entries = @(Get-IcoEntries -Path $Path)
    if ($entries.Count -lt $MinEntryCount) {
        throw "ICO file '$Path' has $($entries.Count) entries; expected at least $MinEntryCount"
    }

    $availableSquareSizes = @(
        $entries |
        Where-Object { $_.Width -eq $_.Height } |
        ForEach-Object { [int]$_.Width } |
        Select-Object -Unique
    )

    foreach ($requiredSize in $RequiredSizes) {
        if ($requiredSize -notin $availableSquareSizes) {
            throw "ICO file '$Path' is missing required icon size ${requiredSize}x${requiredSize}"
        }
    }

    foreach ($entry in $entries) {
        if ($entry.BytesInRes -le 0) {
            throw "ICO file '$Path' contains an empty image payload entry"
        }
    }

    # Match metadata emitted by known-good Cargo-generated tray.ico entries.
    foreach ($entry in $entries) {
        if ($entry.Reserved -ne 0) {
            throw "ICO file '$Path' has non-zero reserved field for entry size $($entry.Width)x$($entry.Height)"
        }
        if ($entry.Planes -ne 0) {
            throw "ICO file '$Path' has unsupported planes field=$($entry.Planes) for entry size $($entry.Width)x$($entry.Height)"
        }
        if ($entry.BitCount -ne 32) {
            throw "ICO file '$Path' has unsupported bitcount field=$($entry.BitCount) for entry size $($entry.Width)x$($entry.Height)"
        }
    }
}

function Test-BinaryContainsAsciiText {
    param(
        [string]$Path,
        [string]$Text
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        throw "Cannot scan missing file for marker: $Path"
    }

    $pattern = [System.Text.Encoding]::ASCII.GetBytes($Text)
    if ($pattern.Length -eq 0) {
        return $true
    }

    $stream = [System.IO.File]::OpenRead($Path)
    try {
        $matchIndex = 0
        while ($true) {
            $nextByte = $stream.ReadByte()
            if ($nextByte -eq -1) {
                break
            }

            if ($nextByte -eq $pattern[$matchIndex]) {
                $matchIndex++
                if ($matchIndex -eq $pattern.Length) {
                    return $true
                }
                continue
            }

            if ($nextByte -eq $pattern[0]) {
                $matchIndex = 1
            }
            else {
                $matchIndex = 0
            }
        }

        return $false
    }
    finally {
        $stream.Dispose()
    }
}

function Assert-BinaryHasNoDummyMarker {
    param(
        [string]$Path,
        [string[]]$Markers
    )

    foreach ($marker in $Markers) {
        if (Test-BinaryContainsAsciiText -Path $Path -Text $marker) {
            throw "Detected dummy Squirrel marker in '$Path': '$marker'."
        }
    }
}

function Wait-FileReadable {
    param(
        [string]$Path,
        [int]$TimeoutSeconds = 60,
        [int]$PollMilliseconds = 250
    )

    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)

    while ([DateTime]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath $Path) {
            try {
                $stream = [System.IO.File]::Open(
                    $Path,
                    [System.IO.FileMode]::Open,
                    [System.IO.FileAccess]::Read,
                    [System.IO.FileShare]::ReadWrite
                )
                try {
                    if ($stream.Length -gt 0) {
                        return
                    }
                }
                finally {
                    $stream.Dispose()
                }
            }
            catch {
                # Keep polling until the file is no longer exclusively locked.
            }
        }

        Start-Sleep -Milliseconds $PollMilliseconds
    }

    throw "Timed out waiting for readable file: $Path"
}

$repoRoot = Resolve-RepoRoot
$binaryFullPath = Join-Path $repoRoot $BinaryPath
$outputFullPath = Join-Path $repoRoot $OutputDir
$workRoot = Join-Path $repoRoot "dist\squirrel-work"
$inputDir = Join-Path $workRoot "input"
$pkgDir = Join-Path $workRoot "pkg"
$toolsDir = Join-Path $workRoot "tools"
$nuspecPath = Join-Path $repoRoot "packaging\windows\squirrel\aura.nuspec"
$packageIconSourcePath = Join-Path $repoRoot "assets\tray.png"

if (-not (Test-Path -LiteralPath $binaryFullPath)) {
    throw "Binary does not exist: $binaryFullPath"
}

if (-not (Test-Path -LiteralPath $nuspecPath)) {
    throw "Nuspec does not exist: $nuspecPath"
}
if (-not (Test-Path -LiteralPath $packageIconSourcePath)) {
    throw "Package icon source does not exist: $packageIconSourcePath"
}

# Execute cleanup prior to resolving and downloading executables
if (Test-Path -LiteralPath $workRoot) {
    Remove-Item -LiteralPath $workRoot -Recurse -Force
}
if (Test-Path -LiteralPath $outputFullPath) {
    Remove-Item -LiteralPath $outputFullPath -Recurse -Force
}

# Recreate required directories
New-Item -ItemType Directory -Path $inputDir -Force | Out-Null
New-Item -ItemType Directory -Path $pkgDir -Force | Out-Null
New-Item -ItemType Directory -Path $outputFullPath -Force | Out-Null

$nugetPath = Resolve-NuGetCommand -CommandName $NuGetExe
$squirrelPath = Resolve-SquirrelExecutable -ProvidedPath $SquirrelExe -NuGetPath $nugetPath -ToolsDir $toolsDir -PackageVersion $SquirrelWindowsVersion
$squirrelVersionInfo = (Get-Item -LiteralPath $squirrelPath).VersionInfo
Write-Host "Using Squirrel.Windows package version: $SquirrelWindowsVersion"
Write-Host "Using Squirrel executable: $squirrelPath"
if ($squirrelVersionInfo) {
    Write-Host ("Squirrel executable version: FileVersion={0}; ProductVersion={1}" -f $squirrelVersionInfo.FileVersion, $squirrelVersionInfo.ProductVersion)
}

Copy-Item -LiteralPath $binaryFullPath -Destination (Join-Path $inputDir "aura.exe") -Force
Copy-Item -LiteralPath $packageIconSourcePath -Destination (Join-Path $inputDir "tray.png") -Force
$setupIconPath = Join-Path $inputDir "tray.ico"
Convert-PngToIco -SourcePath (Join-Path $inputDir "tray.png") -DestinationPath $setupIconPath -Size 256
Assert-IcoHasRequiredSizes -Path $setupIconPath -RequiredSizes @(16, 32, 48, 256) -MinEntryCount 6
$appIconPath = Join-Path $inputDir "app.ico"
Convert-PngToIco -SourcePath (Join-Path $inputDir "tray.png") -DestinationPath $appIconPath -Size 256
Assert-IcoHasRequiredSizes -Path $appIconPath -RequiredSizes @(16, 32, 48, 256) -MinEntryCount 6

& $nugetPath pack $nuspecPath -Version $Version -BasePath $inputDir -OutputDirectory $pkgDir -NoPackageAnalysis -NonInteractive
if ($LASTEXITCODE -ne 0) {
    throw "NuGet pack failed."
}

$nupkgPath = Join-Path $pkgDir ("aura.{0}.nupkg" -f $Version)
if (-not (Test-Path -LiteralPath $nupkgPath)) {
    $candidatePackage = Get-ChildItem -LiteralPath $pkgDir -Filter "*.nupkg" | Select-Object -First 1
    if ($candidatePackage) {
        $nupkgPath = $candidatePackage.FullName
    }
    else {
        $nupkgPath = ""
    }
}
if (-not $nupkgPath) {
    throw "No NuGet package was generated."
}

$squirrelArgs = @(
    "--releasify",
    $nupkgPath,
    "--releaseDir",
    $outputFullPath,
    "--setupIcon",
    $setupIconPath,
    "--no-msi"
)
$squirrelProcess = Start-Process -FilePath $squirrelPath -ArgumentList $squirrelArgs -PassThru -Wait
if ($squirrelProcess.ExitCode -ne 0) {
    throw "Squirrel releasify failed with exit code $($squirrelProcess.ExitCode)."
}

$setupPath = Join-Path $outputFullPath "Setup.exe"
Wait-FileReadable -Path $setupPath -TimeoutSeconds 90 -PollMilliseconds 250

$releasesPath = Join-Path $outputFullPath "RELEASES"
if (-not (Test-Path -LiteralPath $releasesPath)) {
    throw "Squirrel RELEASES file was not generated: $releasesPath"
}

$releasePackages = @(Get-ChildItem -LiteralPath $outputFullPath -Filter "*.nupkg" -File)

if (-not $releasePackages -or $releasePackages.Count -lt 1) {
    throw "No Squirrel release .nupkg files were generated in '$outputFullPath'."
}

$templateSetupPath = Join-Path (Split-Path -Parent $squirrelPath) "Setup.exe"
if (-not (Test-Path -LiteralPath $templateSetupPath)) {
    throw "Squirrel template Setup.exe was not found: $templateSetupPath"
}

$setupHash = (Get-FileHash -LiteralPath $setupPath -Algorithm SHA256).Hash
$templateSetupHash = (Get-FileHash -LiteralPath $templateSetupPath -Algorithm SHA256).Hash
if ($setupHash -eq $templateSetupHash) {
    throw "Generated Setup.exe matches Squirrel template Setup.exe; releasify output was not embedded."
}

$setupSize = (Get-Item -LiteralPath $setupPath).Length
$templateSetupSize = (Get-Item -LiteralPath $templateSetupPath).Length
$largestReleasePackage = $releasePackages | Sort-Object Length -Descending | Select-Object -First 1
$minimumExpectedSetupSize = [Math]::Max(
    $templateSetupSize + 65536,
    [int][Math]::Floor($largestReleasePackage.Length * 0.20)
)
if ($setupSize -lt $minimumExpectedSetupSize) {
    throw "Generated Setup.exe appears too small ($setupSize bytes). Expected at least $minimumExpectedSetupSize bytes based on release package size $($largestReleasePackage.Length)."
}

$dummyMarkers = @(
    "This is a dummy update,exe",
    "This is a dummy update.exe"
)
Assert-BinaryHasNoDummyMarker -Path $setupPath -Markers $dummyMarkers

$updateExePath = Join-Path $outputFullPath "Update.exe"
if (Test-Path -LiteralPath $updateExePath) {
    Assert-BinaryHasNoDummyMarker -Path $updateExePath -Markers $dummyMarkers
}
else {
    Write-Host "Update.exe was not emitted to release root; skipping marker scan for Update.exe."
}

$versionedSetup = Join-Path $outputFullPath ("aura-{0}-windows-installer.exe" -f $Version)
Copy-Item -LiteralPath $setupPath -Destination $versionedSetup -Force
$versionedSetupHash = (Get-FileHash -LiteralPath $versionedSetup -Algorithm SHA256).Hash
if ($versionedSetupHash -ne $setupHash) {
    throw "Versioned installer hash mismatch; copied installer does not match Setup.exe."
}
$versionedInstallerZip = Join-Path $outputFullPath ("aura-{0}-windows-installer.zip" -f $Version)
Compress-Archive -Path $versionedSetup -DestinationPath $versionedInstallerZip -Force

Write-Host "Squirrel packaging complete."
Write-Host "Output directory: $outputFullPath"

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

& $squirrelPath --releasify $nupkgPath --releaseDir $outputFullPath --no-msi
if ($LASTEXITCODE -ne 0) {
    throw "Squirrel releasify failed."
}

$setupPath = Join-Path $outputFullPath "Setup.exe"

# Polling loop to mitigate file system / Antivirus locking race conditions
$maxRetries = 10
$retryCount = 0
$setupExists = $false

while (-not $setupExists -and $retryCount -lt $maxRetries) {
    if (Test-Path -LiteralPath $setupPath) {
        $setupExists = $true
    }
    else {
        Start-Sleep -Milliseconds 500
        $retryCount++
    }
}

if (-not $setupExists) {
    throw "Squirrel setup executable was not found. It may not have generated, or it remains locked by an external process."
}

$releasesPath = Join-Path $outputFullPath "RELEASES"
if (-not (Test-Path -LiteralPath $releasesPath)) {
    throw "Squirrel RELEASES file was not generated: $releasesPath"
}

$releasePackages = @(Get-ChildItem -LiteralPath $outputFullPath -Filter "*.nupkg" -File)

if (-not $releasePackages -or $releasePackages.Count -lt 1) {
    throw "No Squirrel release .nupkg files were generated in '$outputFullPath'."
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
$versionedInstallerZip = Join-Path $outputFullPath ("aura-{0}-windows-installer.zip" -f $Version)
Compress-Archive -Path $versionedSetup -DestinationPath $versionedInstallerZip -Force

Write-Host "Squirrel packaging complete."
Write-Host "Output directory: $outputFullPath"

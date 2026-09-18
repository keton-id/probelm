[CmdletBinding()]
param(
    [string]$Version = $env:PROBELM_VERSION,
    [string]$InstallDir = "$HOME\bin",
    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"
$Repository = "keton-id/probelm"
$PrimaryBinary = "probelm.exe"
$CompatibilityBinary = "mtest.exe"

function Write-Info([string]$Message) { Write-Host "==> $Message" -ForegroundColor Cyan }
function Write-Ok([string]$Message) { Write-Host "OK  $Message" -ForegroundColor Green }

$primaryPath = Join-Path $InstallDir $PrimaryBinary
$compatibilityPath = Join-Path $InstallDir $CompatibilityBinary

if ($Uninstall) {
    Remove-Item $primaryPath, $compatibilityPath -Force -ErrorAction SilentlyContinue
    Write-Ok "Removed probelm from $InstallDir"
    exit 0
}

$architecture = switch ($env:PROCESSOR_ARCHITECTURE) {
    "ARM64" { "aarch64"; break }
    "AMD64" { "x86_64"; break }
    default { throw "Unsupported Windows architecture: $env:PROCESSOR_ARCHITECTURE" }
}

if ([string]::IsNullOrWhiteSpace($Version)) {
    $release = Invoke-RestMethod -Headers @{ "User-Agent" = "probelm-installer" } `
        -Uri "https://api.github.com/repos/$Repository/releases/latest"
    $Version = $release.tag_name
}
if (-not $Version.StartsWith("v")) { $Version = "v$Version" }

$asset = "probelm-windows-$architecture.zip"
$baseUrl = "https://github.com/$Repository/releases/download/$Version"
$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) "probelm-$([Guid]::NewGuid())"
$archive = Join-Path $tempRoot $asset
$checksumFile = "$archive.sha256"
$extractDir = Join-Path $tempRoot "extracted"

try {
    New-Item -ItemType Directory -Force -Path $tempRoot, $extractDir | Out-Null
    Write-Info "Downloading $asset ($Version)"
    Invoke-WebRequest -Uri "$baseUrl/$asset" -OutFile $archive
    Invoke-WebRequest -Uri "$baseUrl/$asset.sha256" -OutFile $checksumFile

    $expected = (Get-Content $checksumFile -Raw).Trim().Split()[0].ToLowerInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 -Path $archive).Hash.ToLowerInvariant()
    if ($expected -notmatch '^[0-9a-f]{64}$' -or $expected -ne $actual) {
        throw "SHA-256 verification failed for $asset"
    }

    Expand-Archive -LiteralPath $archive -DestinationPath $extractDir -Force
    $downloadedBinary = Join-Path $extractDir $PrimaryBinary
    if (-not (Test-Path $downloadedBinary)) { throw "Release archive does not contain $PrimaryBinary" }

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item $downloadedBinary $primaryPath -Force
    Copy-Item $downloadedBinary $compatibilityPath -Force
    Write-Ok "Installed probelm and the existing mtest compatibility command to $InstallDir"

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User") -split ';'
    if ($userPath -notcontains $InstallDir) {
        Write-Host "Add this directory to your user PATH: $InstallDir" -ForegroundColor Yellow
    }
} finally {
    Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
}

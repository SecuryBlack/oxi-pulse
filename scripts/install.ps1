# OxiPulse — Windows Install Script
# Usage (generic):     irm https://install.oxipulse.dev | iex
# Usage (SecuryBlack): irm https://install.oxipulse.dev | iex -Endpoint ingest.securyblack.com -Token <TOKEN>
#
# Or with explicit params:
#   $script = irm https://install.oxipulse.dev
#   & ([scriptblock]::Create($script)) -Endpoint "https://ingest.example.com:4317" -Token "tok_abc123"
[CmdletBinding()]
param(
    [string]$Endpoint = "",
    [string]$Token    = "",
    [string]$Mode     = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$SbAgentLabel = "oxipulse"
$libUrl = "https://raw.githubusercontent.com/securyblack/sb-agent-core/main/scripts/install-lib.ps1"
$libTmp = Join-Path ([System.IO.Path]::GetTempPath()) "sb-agent-core-install-lib.ps1"
Invoke-WebRequest -Uri $libUrl -OutFile $libTmp -UseBasicParsing
. $libTmp

# ─── Constants ────────────────────────────────────────────────────────────────
$GithubRepo  = "securyblack/oxi-pulse"
$BinaryName  = "oxipulse.exe"
$InstallDir  = "$env:ProgramFiles\OxiPulse"
$ConfigDir   = "$env:ProgramData\oxipulse"
$ConfigFile  = "$ConfigDir\config.toml"
$ServiceName = "OxiPulse"

# ─── Banner ───────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "  OxiPulse — Server Monitoring Agent" -ForegroundColor Cyan -NoNewline
Write-Host " (Windows Installer)" -ForegroundColor Gray
Write-Host ""

Assert-SbAdmin
$target = Get-SbArchTarget
$version = Get-SbLatestVersion -GithubRepo $GithubRepo

$tmpDir = [System.IO.Path]::GetTempPath() + [System.IO.Path]::GetRandomFileName()
New-Item -ItemType Directory -Path $tmpDir | Out-Null

try {
    $assetName = "oxipulse-$target.zip"
    $zipPath = Get-SbReleaseAsset -GithubRepo $GithubRepo -Version $version -AssetName $assetName -TmpDir $tmpDir
    Install-SbBinaryFromZip -ZipPath $zipPath -BinaryName $BinaryName -InstallDir $InstallDir -ServiceName $ServiceName

    # ─── Configuration ────────────────────────────────────────────────────────
    New-Item -ItemType Directory -Path $ConfigDir -Force | Out-Null

    if ($Mode -eq "local_agent") {
        if (-not $Endpoint) { $Endpoint = "http://localhost:4317" }
        Write-SbInfo "Mode: local_agent — OxiPulse will send metrics to localhost:4317"
    }

    if (-not $Endpoint) {
        Write-Host ""
        $Endpoint = Read-Host "  OTLP endpoint (e.g. https://ingest.example.com:4317)"
    }
    if (-not $Token) {
        $secToken = Read-Host "  Auth token" -AsSecureString
        $Token    = [Runtime.InteropServices.Marshal]::PtrToStringAuto(
                        [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secToken))
    }

    if (-not $Endpoint) { Invoke-SbFail "Endpoint cannot be empty" }
    if (-not $Token)    { Invoke-SbFail "Token cannot be empty" }

    Write-SbInfo "Writing config to $ConfigFile..."
    $effectiveMode = if ($Mode) { $Mode } else { "direct" }
    @"
# OxiPulse configuration
# Do not share this file — it contains your auth token.
version = "$version"
mode = "$effectiveMode"
endpoint = "$Endpoint"
token = "$Token"
interval_secs = 30
buffer_max_size = 8640
"@ | Set-Content -Path $ConfigFile -Encoding UTF8

    Protect-SbConfigFile -Path $ConfigFile
    Write-SbSuccess "Config written"

    # ─── Windows Service ──────────────────────────────────────────────────────
    Register-SbWindowsService -ServiceName $ServiceName -DisplayName "OxiPulse Monitoring Agent" `
        -BinaryPath "$InstallDir\$BinaryName" `
        -Description "Ultralight server monitoring agent. See https://github.com/$GithubRepo"

} finally {
    Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
}

# ─── Done ─────────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "  OxiPulse $version installed successfully!" -ForegroundColor Green
Write-Host ""
Write-Host "  Status:  " -NoNewline; Write-Host "Get-Service OxiPulse" -ForegroundColor White
Write-Host "  Logs:    " -NoNewline; Write-Host "Get-EventLog -LogName Application -Source OxiPulse -Newest 50" -ForegroundColor White
Write-Host "  Config:  " -NoNewline; Write-Host $ConfigFile -ForegroundColor White
Write-Host ""

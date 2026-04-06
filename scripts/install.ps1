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
    [string]$Token    = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# ─── Helpers ──────────────────────────────────────────────────────────────────
function Write-Info    { param($msg) Write-Host "[oxipulse] $msg" -ForegroundColor Cyan }
function Write-Success { param($msg) Write-Host "[oxipulse] $msg" -ForegroundColor Green }
function Write-Warn    { param($msg) Write-Host "[oxipulse] $msg" -ForegroundColor Yellow }
function Fail          { param($msg) Write-Host "[oxipulse] ERROR: $msg" -ForegroundColor Red; exit 1 }

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

# ─── Admin check ──────────────────────────────────────────────────────────────
$currentPrincipal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $currentPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Fail "This script must be run as Administrator. Right-click PowerShell and select 'Run as Administrator'."
}

# ─── Architecture detection ───────────────────────────────────────────────────
# Use PROCESSOR_ARCHITECTURE env var — works on all Windows versions including
# Server 2025 with PowerShell 5.1 (.NET Framework), where RuntimeInformation
# may not expose OSArchitecture.
$procArch = $env:PROCESSOR_ARCHITECTURE
$target = switch ($procArch) {
    "AMD64" { "x86_64-pc-windows-msvc" }
    "ARM64" { "aarch64-pc-windows-msvc" }
    default { Fail "Unsupported architecture: $procArch" }
}

Write-Info "Detected architecture: $procArch ($target)"

# ─── Resolve latest release version ──────────────────────────────────────────
Write-Info "Fetching latest release from GitHub..."
$releaseApi  = "https://api.github.com/repos/$GithubRepo/releases/latest"
$releaseInfo = Invoke-RestMethod -Uri $releaseApi -Headers @{ "User-Agent" = "oxipulse-installer" }
$version     = $releaseInfo.tag_name

if (-not $version) { Fail "Could not determine latest version. Check your internet connection." }

Write-Info "Latest version: $version"

# ─── Download binary ──────────────────────────────────────────────────────────
$assetName   = "oxipulse-$target.zip"
$downloadUrl = "https://github.com/$GithubRepo/releases/download/$version/$assetName"
$checksumUrl = "$downloadUrl.sha256"
$tmpDir      = [System.IO.Path]::GetTempPath() + [System.IO.Path]::GetRandomFileName()
New-Item -ItemType Directory -Path $tmpDir | Out-Null

try {
    Write-Info "Downloading $assetName..."
    $zipPath = "$tmpDir\$assetName"
    Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath -UseBasicParsing

    # Verify checksum if available
    try {
        $checksumFile = "$tmpDir\$assetName.sha256"
        Invoke-WebRequest -Uri $checksumUrl -OutFile $checksumFile -UseBasicParsing
        $expected = (Get-Content $checksumFile).Split(" ")[0].Trim().ToLower()
        $actual   = (Get-FileHash -Algorithm SHA256 $zipPath).Hash.ToLower()
        if ($expected -ne $actual) { Fail "Checksum mismatch. Download may be corrupted." }
        Write-Success "Checksum OK"
    } catch {
        Write-Warn "No checksum file found, skipping verification"
    }

    # ─── Install binary ───────────────────────────────────────────────────────
    Write-Info "Installing binary to $InstallDir..."
    Expand-Archive -Path $zipPath -DestinationPath $tmpDir -Force
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item "$tmpDir\$BinaryName" "$InstallDir\$BinaryName" -Force
    Write-Success "Binary installed"

    # ─── Configuration ────────────────────────────────────────────────────────
    New-Item -ItemType Directory -Path $ConfigDir -Force | Out-Null

    if (-not $Endpoint) {
        Write-Host ""
        $Endpoint = Read-Host "  OTLP endpoint (e.g. https://ingest.example.com:4317)"
    }
    if (-not $Token) {
        $secToken = Read-Host "  Auth token" -AsSecureString
        $Token    = [Runtime.InteropServices.Marshal]::PtrToStringAuto(
                        [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secToken))
    }

    if (-not $Endpoint) { Fail "Endpoint cannot be empty" }
    if (-not $Token)    { Fail "Token cannot be empty" }

    Write-Info "Writing config to $ConfigFile..."
    @"
# OxiPulse configuration
# Do not share this file — it contains your auth token.
endpoint = "$Endpoint"
token = "$Token"
interval_secs = 10
buffer_max_size = 8640
"@ | Set-Content -Path $ConfigFile -Encoding UTF8

    # Restrict config file permissions to Administrators and SYSTEM only.
    # Use well-known SIDs (locale-independent) instead of string account names
    # to avoid "identity reference could not be translated" on non-English Windows.
    $acl = Get-Acl $ConfigFile
    $acl.SetAccessRuleProtection($true, $false)
    $adminSid   = New-Object System.Security.Principal.SecurityIdentifier(
        [System.Security.Principal.WellKnownSidType]::BuiltinAdministratorsSid, $null)
    $systemSid  = New-Object System.Security.Principal.SecurityIdentifier(
        [System.Security.Principal.WellKnownSidType]::LocalSystemSid, $null)
    $adminRule  = New-Object System.Security.AccessControl.FileSystemAccessRule(
        $adminSid, "FullControl", "Allow")
    $systemRule = New-Object System.Security.AccessControl.FileSystemAccessRule(
        $systemSid, "FullControl", "Allow")
    $acl.AddAccessRule($adminRule)
    $acl.AddAccessRule($systemRule)
    Set-Acl -Path $ConfigFile -AclObject $acl
    Write-Success "Config written"

    # ─── Windows Service ──────────────────────────────────────────────────────
    Write-Info "Registering Windows Service '$ServiceName'..."

    # Remove existing service if present
    if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
        Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
        & sc.exe delete $ServiceName | Out-Null
        Start-Sleep -Seconds 1
    }

    $binPath = "$InstallDir\$BinaryName"
    New-Service -Name $ServiceName `
                -BinaryPathName $binPath `
                -DisplayName "OxiPulse Monitoring Agent" `
                -Description "Ultralight server monitoring agent. See https://github.com/$GithubRepo" `
                -StartupType Automatic | Out-Null

    # Configure restart on failure (sc.exe failure config)
    & sc.exe failure $ServiceName reset= 86400 actions= restart/10000/restart/30000/restart/60000 | Out-Null
    # Trigger failure actions even on clean exit (exit code 0).
    # This is required for auto-update: the agent exits cleanly after replacing
    # its binary, and the SCM must restart it to pick up the new version.
    & sc.exe failureflag $ServiceName 1 | Out-Null

    Start-Service -Name $ServiceName
    Write-Success "Service registered and started"

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

# from-vaultcompass.ps1 - carry a VaultCompass installation's data over to Folioneer (Windows).
#
# One-off, run once per computer with the application closed (todo #033). Folioneer keeps
# its data under a new identifier, so a fresh Folioneer opens empty until this has run.
#
# It copies, never moves: the database (with its write-ahead log), the interface's saved
# preferences and the window position go to the new folders; logs and caches stay behind.
# The old folder is left exactly as it is, as a backup to delete by hand later. An existing
# Folioneer database is never overwritten. The scheduled download registered under the old
# name is removed; Folioneer registers its own at its next start.
#
# Use, from a PowerShell window:
#   powershell -ExecutionPolicy Bypass -File from-vaultcompass.ps1
# For a rehearsal on copies: -LocalAppData DIR -AppData DIR -NoScheduler -NoRunningCheck

param(
    [string]$LocalAppData = $env:LOCALAPPDATA,
    [string]$AppData = $env:APPDATA,
    [switch]$NoScheduler,
    [switch]$NoRunningCheck
)

$ErrorActionPreference = "Stop"

$OldId = "com.phileggel.vault-compass"
$NewId = "com.folioneer.desktop"
$OldTask = "VaultCompassFetch"

$oldData = Join-Path $LocalAppData $OldId
$newData = Join-Path $LocalAppData $NewId
$oldConfig = Join-Path $AppData $OldId
$newConfig = Join-Path $AppData $NewId

if (-not (Test-Path (Join-Path $oldData "portfolio"))) {
    Write-Host "No VaultCompass database at $oldData - nothing to carry over."
    exit 0
}

if (Test-Path (Join-Path $newData "portfolio")) {
    Write-Host "Folioneer already has a database at $newData\portfolio - nothing was copied." -ForegroundColor Red
    Write-Host "If Folioneer was opened before this script and holds nothing you want, close it,"
    Write-Host "delete $newData, and run this script again."
    exit 1
}

if (-not $NoRunningCheck) {
    foreach ($name in @("tauri-app", "vault-compass", "folioneer")) {
        if (Get-Process -Name $name -ErrorAction SilentlyContinue) {
            Write-Host "The application is running ($name). Close it and run this script again." -ForegroundColor Red
            exit 1
        }
    }
}

New-Item -ItemType Directory -Force -Path $newData | Out-Null

# The write-ahead log travels with the database it belongs to; the database file goes last,
# so an interrupted run leaves no database behind and can simply be run again.
foreach ($file in @("portfolio-wal", "portfolio-shm", "portfolio")) {
    $source = Join-Path $oldData $file
    if (Test-Path $source) {
        Copy-Item -Path $source -Destination (Join-Path $newData $file)
    }
}

# The interface's saved preferences live in the embedded browser's profile; its caches do not
# need to travel.
$oldProfile = Join-Path $oldData "EBWebView"
if (Test-Path $oldProfile) {
    $newProfile = Join-Path $newData "EBWebView"
    & robocopy $oldProfile $newProfile /E /XD "Cache" "Code Cache" "GPUCache" "CacheStorage" "ShaderCache" "GrShaderCache" /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -ge 8) {
        Write-Host "Copying the interface preferences failed (robocopy $LASTEXITCODE). The database was copied; preferences will be the defaults." -ForegroundColor Yellow
    }
}

$oldWindowState = Join-Path $oldConfig ".window-state.json"
if (Test-Path $oldWindowState) {
    New-Item -ItemType Directory -Force -Path $newConfig | Out-Null
    Copy-Item -Path $oldWindowState -Destination (Join-Path $newConfig ".window-state.json")
}

foreach ($file in @("portfolio", "portfolio-wal", "portfolio-shm")) {
    $source = Join-Path $oldData $file
    if (Test-Path $source) {
        $before = (Get-FileHash -Algorithm SHA256 -Path $source).Hash
        $after = (Get-FileHash -Algorithm SHA256 -Path (Join-Path $newData $file)).Hash
        if ($before -ne $after) {
            Write-Host "$file differs from its copy - was the application running? Delete $newData and run again." -ForegroundColor Red
            exit 1
        }
    }
}

if (-not $NoScheduler) {
    # Through cmd: Windows PowerShell turns a native command's redirected error output into a
    # terminating error, and a task that does not exist is the normal case here.
    & cmd /c "schtasks /Query /TN $OldTask >nul 2>&1"
    if ($LASTEXITCODE -eq 0) {
        & cmd /c "schtasks /Delete /TN $OldTask /F >nul 2>&1"
        Write-Host "The scheduled download registered as $OldTask was removed." -ForegroundColor Green
    }
}

Write-Host "Carried over to $newData" -ForegroundColor Green
Write-Host "Left in place, untouched: $oldData (your backup - delete it once Folioneer has run for a while)."
Write-Host "Next: uninstall VaultCompass from Windows Settings > Apps, leaving 'delete application data' unticked;"
Write-Host "install Folioneer, open it, check your accounts. If the daily price download was on,"
Write-Host "it registers itself again at that first start (Settings shows it)."

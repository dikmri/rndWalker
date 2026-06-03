param(
    [string]$Version = "latest",
    [string]$InstallDir = "",
    [switch]$NoPath,
    [switch]$NoShortcut
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

if ([Net.ServicePointManager]::SecurityProtocol -band [Net.SecurityProtocolType]::Tls12) {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} else {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
}

function Write-Step {
    param([string]$Message)
    Write-Host "[rndWalker] $Message"
}

function Get-NormalizedTag {
    param([string]$InputVersion)
    if ($InputVersion.StartsWith("v")) {
        return $InputVersion
    }
    return "v$InputVersion"
}

function Clear-InstallDirectory {
    param([string]$Path)

    $root = [System.IO.Path]::GetPathRoot($Path)
    if ([string]::IsNullOrWhiteSpace($Path) -or $Path.TrimEnd("\") -eq $root.TrimEnd("\")) {
        throw "Refusing to install into an unsafe directory: $Path"
    }

    if (Test-Path -LiteralPath $Path) {
        Get-ChildItem -LiteralPath $Path -Force | Remove-Item -Recurse -Force
    } else {
        New-Item -ItemType Directory -Force -Path $Path | Out-Null
    }
}

if (-not [Environment]::Is64BitOperatingSystem) {
    throw "rndWalker currently provides Windows x64 releases only."
}

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $localAppData = if ($env:LOCALAPPDATA) { $env:LOCALAPPDATA } else { Join-Path $HOME "AppData\Local" }
    $InstallDir = Join-Path $localAppData "Programs\rndWalker"
}

$InstallDir = [Environment]::ExpandEnvironmentVariables($InstallDir)
$InstallDir = [System.IO.Path]::GetFullPath($InstallDir)

$repo = "dikmri/rndWalker"
$headers = @{
    "Accept" = "application/vnd.github+json"
    "User-Agent" = "rndWalker-installer"
}

if ($Version -eq "latest") {
    Write-Step "Fetching latest release metadata"
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest" -Headers $headers
    $tag = $release.tag_name
} else {
    $tag = Get-NormalizedTag $Version
    Write-Step "Fetching release metadata for $tag"
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/tags/$tag" -Headers $headers
}

if ([string]::IsNullOrWhiteSpace($tag)) {
    throw "Could not determine the release tag."
}

$assetName = "rndWalker-$tag-x86_64-pc-windows-gnu.zip"
$asset = $release.assets | Where-Object { $_.name -eq $assetName } | Select-Object -First 1
if (-not $asset) {
    throw "Release asset not found: $assetName"
}

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) "rndWalker-install-$([guid]::NewGuid())"
$zipPath = Join-Path $tempRoot $assetName
$extractDir = Join-Path $tempRoot "extract"

try {
    New-Item -ItemType Directory -Force -Path $extractDir | Out-Null

    Write-Step "Downloading $assetName"
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zipPath -Headers $headers

    Write-Step "Extracting archive"
    Expand-Archive -LiteralPath $zipPath -DestinationPath $extractDir -Force

    Write-Step "Installing to $InstallDir"
    Clear-InstallDirectory $InstallDir
    Copy-Item -Path (Join-Path $extractDir "*") -Destination $InstallDir -Recurse -Force

    $exePath = Join-Path $InstallDir "rndWalker.exe"
    if (-not (Test-Path -LiteralPath $exePath)) {
        throw "Installed executable was not found: $exePath"
    }

    if (-not $NoShortcut) {
        $shortcutDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\rndWalker"
        New-Item -ItemType Directory -Force -Path $shortcutDir | Out-Null
        $shortcutPath = Join-Path $shortcutDir "rndWalker.lnk"
        $shell = New-Object -ComObject WScript.Shell
        $shortcut = $shell.CreateShortcut($shortcutPath)
        $shortcut.TargetPath = $exePath
        $shortcut.WorkingDirectory = $InstallDir
        $shortcut.Save()
    }

    if (-not $NoPath) {
        $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
        $pathParts = @()
        if (-not [string]::IsNullOrWhiteSpace($userPath)) {
            $pathParts = $userPath -split ";" | ForEach-Object { $_.TrimEnd("\") }
        }

        if (-not ($pathParts | Where-Object { $_ -ieq $InstallDir.TrimEnd("\") })) {
            $newPath = if ([string]::IsNullOrWhiteSpace($userPath)) { $InstallDir } else { "$userPath;$InstallDir" }
            [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
            Write-Step "Added install directory to the user PATH. Open a new terminal to use rndWalker from PATH."
        }
    }

    Write-Step "Installed $tag"
    Write-Host "Executable: $exePath"
} finally {
    if (Test-Path -LiteralPath $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force
    }
}

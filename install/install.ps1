# SPDX-License-Identifier: GPL-3.0-or-later
#
# VeilVoice installer for Windows.
#
#   powershell -ExecutionPolicy Bypass -File install.ps1
#   .\install.ps1 -Yes                  # no prompts, no optional components
#   .\install.ps1 -Version v0.1.9
#   .\install.ps1 -Prefix "D:\Tools\VeilVoice"
#
# ---------------------------------------------------------------------------
# What this script will and will not do
# ---------------------------------------------------------------------------
#
# It downloads a release, proves it is the one this project published, and puts
# it on your PATH. It refuses -- naming the check that failed -- rather than
# continuing past anything it could not verify. There is no switch to skip
# verification, because an installer with one is an installer whose
# verification is decorative.
#
# It installs nothing else unless you say so. VB-CABLE, Audacity and GnuPG are
# offered once, as explicit questions that default to **no**. With -Yes they
# are not installed at all: -Yes means "do not ask me", and answering an
# unasked question by installing software on somebody's machine is exactly the
# behaviour that makes install scripts untrustworthy.
#
# ---------------------------------------------------------------------------
# The order of the checks, which is the whole point
# ---------------------------------------------------------------------------
#
#   1. Fetch the public key and check its fingerprint against the constant
#      below, which is hardcoded in this file. This is the only anchor in the
#      process; everything after it is only as good as this comparison.
#   2. Verify the detached signature over SHA256SUMS with that key.
#   3. Verify the archive's SHA-256 against the now-trusted SHA256SUMS.
#   4. Only then unpack and install.
#
# The order matters. Checking the hash first proves only that the download
# matches a list that might itself have been replaced; the signature is what
# makes the list worth checking against. Without GnuPG, steps 1 and 2 cannot be
# done at all, and this script stops rather than pretending that a hash checked
# against an unverified list is a security check.

[CmdletBinding()]
param(
    [switch] $Yes,
    [string] $Version = "",
    [string] $Prefix = "",
    [switch] $WithVBCable,
    [switch] $WithAudacity,
    [switch] $WithGpg
)

$ErrorActionPreference = "Stop"

# ---------------------------------------------------------------------------
# The trust anchor. Hardcoded on purpose: if it were fetched, it would not be
# an anchor. Compare it against the fingerprint published in README.md, on
# https://tilas01.github.io/veilvoice/ and in every release's notes.
# ---------------------------------------------------------------------------
$FINGERPRINT = "8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A"

$REPO    = "tilas01/veilvoice"
$KEY_URL = "https://tilas01.github.io/veilvoice/assets/veilvoice-signing-key.asc"
$KEY_URL_FALLBACK = "https://raw.githubusercontent.com/$REPO/main/website/assets/veilvoice-signing-key.asc"
$LABEL = "windows-x86_64"

function Write-Step { param($m) Write-Host "==> $m" -ForegroundColor DarkGray }
function Write-Good { param($m) Write-Host "  ok   $m" -ForegroundColor Green }
function Write-Say  { param($m) Write-Host $m }

# Every refusal goes through here, so every refusal names the check.
function Deny {
    param([string] $Reason, [string[]] $Detail = @())
    Write-Host ""
    Write-Host "REFUSED: $Reason" -ForegroundColor Red
    foreach ($line in $Detail) { Write-Host "  $line" }
    Write-Host ""
    Write-Host "Nothing has been installed."
    exit 1
}

function Test-Have { param($name) return [bool](Get-Command $name -ErrorAction SilentlyContinue) }

# Where to find GnuPG.
#
# On PATH if it is there. Otherwise a fixed list of absolute, well-known
# install locations -- Gpg4win's, and the copy Git for Windows bundles, which a
# great many people already have without knowing it and without it being on
# their PATH. That turns "install Gpg4win first" into "verified" for a large
# share of Windows users.
#
# The locations are absolute and enumerated on purpose. `gpg` as a bare name
# would be resolved through Windows' search order, which includes the current
# working directory ahead of most of PATH -- so running this installer from a
# folder containing a file called gpg.exe would run that instead. That is
# finding F-13 in `docs/AUDIT.md`, in the two modules where it mattered most,
# and the same rule applies with more force here: this is the program that
# decides whether the download is genuine.
function Find-Gpg {
    $onPath = Get-Command "gpg" -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }

    $known = @(
        (Join-Path $env:ProgramFiles "GnuPG\bin\gpg.exe"),
        (Join-Path ${env:ProgramFiles(x86)} "GnuPG\bin\gpg.exe"),
        (Join-Path $env:ProgramFiles "Git\usr\bin\gpg.exe"),
        (Join-Path ${env:ProgramFiles(x86)} "Git\usr\bin\gpg.exe"),
        (Join-Path $env:LOCALAPPDATA "Programs\Git\usr\bin\gpg.exe")
    )
    foreach ($candidate in $known) {
        if ($candidate -and (Test-Path $candidate)) { return $candidate }
    }
    return $null
}

# Git for Windows bundles an MSYS build of GnuPG, and it does not understand
# Windows paths. Given `C:\Users\...` it treats the whole thing as a *relative*
# POSIX path and resolves it against the current directory, producing
# `/c/current/dir/C:\Users\...` and failing with "directory does not exist".
# Measured, not guessed -- it is what the first run of this script did.
#
# So paths handed to an MSYS gpg are translated to `/c/Users/...` form. A
# native Gpg4win build takes Windows paths as they are and must not be
# translated, hence the flag rather than doing it unconditionally.
function Test-MsysGpg {
    param([string] $Path)
    return ($Path -match '\\Git\\usr\\bin\\gpg\.exe$' -or $Path -match '\\usr\\bin\\gpg\.exe$')
}

function ConvertTo-GpgPath {
    param([string] $Path, [bool] $Msys)
    if (-not $Msys) { return $Path }
    $p = $Path -replace '\\', '/'
    if ($p -match '^([A-Za-z]):/(.*)$') { return "/" + $Matches[1].ToLower() + "/" + $Matches[2] }
    return $p
}

# Native commands and `$ErrorActionPreference = "Stop"` do not mix on Windows
# PowerShell 5.1: anything the program writes to stderr is turned into an
# ErrorRecord, which then terminates the script even when the program exited
# successfully. gpg writes progress to stderr as a matter of course. So native
# calls run with the preference relaxed and are judged on their exit code,
# which is the thing that actually says whether they worked.
function Invoke-Gpg {
    param([string] $Exe, [string[]] $GpgArgs)
    $previous = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $Exe @GpgArgs 2>&1
        return [pscustomobject]@{ Code = $LASTEXITCODE; Output = $output }
    } finally {
        $ErrorActionPreference = $previous
    }
}

function Ask {
    param([string] $Question)
    # Defaults to no, every time. A prompt whose default is "yes" is not a
    # question, it is an announcement.
    if ($Yes) { return $false }
    if ([Console]::IsInputRedirected) { return $false }
    $answer = Read-Host "  $Question [y/N]"
    return ($answer -match '^[yY]')
}

# TLS 1.2 on Windows PowerShell 5.1, whose default is still SSL3/TLS1.0 and
# which therefore cannot reach github.com at all without this line.
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

function Get-File {
    param([string] $Url, [string] $Path)
    try {
        # -UseBasicParsing: on 5.1 the default path needs Internet Explorer's
        # engine to be initialised, which fails on a server or a fresh account.
        Invoke-WebRequest -Uri $Url -OutFile $Path -UseBasicParsing
        return $true
    } catch {
        return $false
    }
}

Write-Say ""
Write-Say "  VeilVoice installer"

# ---------------------------------------------------------------------------
# Which version
# ---------------------------------------------------------------------------
if (-not $Version) {
    Write-Step "Asking GitHub for the latest release"
    try {
        $latest = Invoke-RestMethod -Uri "https://api.github.com/repos/$REPO/releases/latest" -UseBasicParsing
        $Version = $latest.tag_name
    } catch {
        Deny "could not reach the GitHub API to find the latest release" @(
            "Pass one explicitly instead:  .\install.ps1 -Version v0.1.9"
        )
    }
}
if (-not $Version) {
    Deny "could not read a tag name from the GitHub API reply" @(
        "Pass one explicitly instead:  .\install.ps1 -Version v0.1.9"
    )
}

$ARCHIVE = "veilvoice-$Version-$LABEL.zip"
$BASE    = "https://github.com/$REPO/releases/download/$Version"

Write-Say "  version    $Version"
Write-Say "  build      $LABEL"
Write-Say "  key        $FINGERPRINT"
Write-Say ""

# Short on purpose. GnuPG's agent puts its socket inside the home directory it
# is given, and a Unix-domain socket path cannot exceed about 108 bytes -- a
# limit the MSYS build of GnuPG that Git for Windows bundles is subject to as
# well. A full GUID here produced a 90-character home directory, the agent
# failed to start with "exit status 2", and the import failed for a reason that
# had nothing to do with the key. Measured: 37 characters worked, 90 did not.
$WORK = Join-Path ([IO.Path]::GetTempPath()) ("vv" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
New-Item -ItemType Directory -Path $WORK -Force | Out-Null

try {
    # -----------------------------------------------------------------------
    # 1. Download
    # -----------------------------------------------------------------------
    Write-Step "Downloading"
    if (-not (Get-File "$BASE/$ARCHIVE" "$WORK\$ARCHIVE")) {
        Deny "could not download $ARCHIVE" @(
            "Checked: $BASE/$ARCHIVE",
            "If that version is not published, the releases page lists what is:",
            "  https://github.com/$REPO/releases"
        )
    }
    Write-Good $ARCHIVE

    if (-not (Get-File "$BASE/SHA256SUMS" "$WORK\SHA256SUMS")) {
        Deny "could not download SHA256SUMS" @(
            "Without the hash list the download cannot be checked, so it will",
            "not be installed."
        )
    }
    Write-Good "SHA256SUMS"

    # The per-file manifest, so the installed program can check every file and
    # not just the archive. Best effort: older releases do not publish it, and
    # it is covered by SHA256SUMS so it cannot be swapped undetected.
    $HaveContents = Get-File "$BASE/CONTENTS.sha256" "$WORK\CONTENTS.sha256"

    if (-not (Get-File "$BASE/SHA256SUMS.asc" "$WORK\SHA256SUMS.asc")) {
        Deny "this release has no signature (SHA256SUMS.asc)" @(
            "Every signed release publishes one. Its absence means either that",
            "this release was built without the signing key, or that you are not",
            "looking at the release you think you are.",
            "",
            "This script will not install an unsigned build."
        )
    }
    Write-Good "SHA256SUMS.asc"

    # -----------------------------------------------------------------------
    # 2. The key, and its fingerprint
    # -----------------------------------------------------------------------
    $gpg = Find-Gpg
    if (-not $gpg) {
        Write-Say ""
        Write-Say "  GnuPG is not installed, and it is what verifies the signature."
        Write-Say "  Without it this script can check that the download matches a"
        Write-Say "  hash list, but not that the hash list is the one VeilVoice"
        Write-Say "  published -- which is not a security check at all."
        Write-Say ""
        if ($WithGpg -or (Ask "Install GnuPG (Gpg4win) now?")) {
            if (Test-Have "winget") {
                winget install --id GnuPG.Gpg4win -e --accept-package-agreements --accept-source-agreements
                $env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" +
                            [Environment]::GetEnvironmentVariable("Path", "User")
            } else {
                Deny "winget is not available to install GnuPG" @(
                    "Install Gpg4win yourself and run this again:",
                    "  https://gpg4win.org/"
                )
            }
        }
        $gpg = Find-Gpg
        if (-not $gpg) {
            Deny "GnuPG is still not available" @(
                "The signature cannot be verified without it, and this script does",
                "not install software it could not verify.",
                "",
                "Install Gpg4win and run this again:  https://gpg4win.org/"
            )
        }
    }

    Write-Step "Checking the signing key's fingerprint"
    # F-79, the other half. `Get-File` swallows the exception, so this script
    # never printed somebody else's error here the way `install.sh` did. What
    # it also never did was say that the first address failed and the second
    # one answered, which is worth a line: on a network that refuses the
    # website and allows the repository, the reader should know which copy of
    # the key was checked.
    if (-not (Get-File $KEY_URL "$WORK\key.asc")) {
        Write-Say "  the website copy could not be fetched; trying the repository copy"
        if (-not (Get-File $KEY_URL_FALLBACK "$WORK\key.asc")) {
            Deny "could not download the public key" @(
                "Tried: $KEY_URL",
                "  and: $KEY_URL_FALLBACK"
            )
        }
    }

    # A throwaway keyring. Importing into the user's own is a side effect
    # nobody asked for, and it would also let a previously-imported key satisfy
    # this check instead of the one just downloaded.
    $msys = Test-MsysGpg $gpg
    $home_ = Join-Path $WORK "g"
    New-Item -ItemType Directory -Path $home_ -Force | Out-Null

    $homeArg = ConvertTo-GpgPath $home_ $msys

    # See the note on $WORK. If TEMP itself is long enough that even a short
    # name overflows the agent's socket path, say so plainly -- the failure
    # would otherwise surface as "the key could not be imported", which is
    # true and completely misleading.
    if ($msys -and $homeArg.Length -gt 80) {
        Deny "the temporary directory path is too long for this build of GnuPG" @(
            "Using: $gpg",
            "Home:  $homeArg ($($homeArg.Length) characters)",
            "",
            "This is the GnuPG bundled with Git for Windows. Its agent puts a",
            "Unix-domain socket inside that directory, and such a path cannot",
            "exceed about 108 bytes, so the agent will not start.",
            "",
            "Install Gpg4win, which is a native Windows build and has no such",
            "limit, then run this again:  https://gpg4win.org/",
            "Or set TEMP to a shorter path first."
        )
    }
    $keyArg  = ConvertTo-GpgPath "$WORK\key.asc" $msys

    $imported = Invoke-Gpg $gpg @("--homedir", $homeArg, "--batch", "--quiet", "--import", $keyArg)
    if ($imported.Code -ne 0) {
        Deny "the downloaded public key could not be imported" @(
            "The file at $KEY_URL is not a valid OpenPGP public key,",
            "or GnuPG could not write its temporary keyring.",
            "",
            ($imported.Output | Out-String).Trim()
        )
    }

    $fprLines = (Invoke-Gpg $gpg @("--homedir", $homeArg, "--batch", "--with-colons", "--fingerprint")).Output
    $got = ""
    foreach ($line in $fprLines) {
        $t = [string]$line
        if ($t -like "fpr:*") {
            $got = ($t -split ":")[9]
            break
        }
    }

    if ($got -ne $FINGERPRINT) {
        $shown = $got
        if (-not $shown) { $shown = "(none)" }
        Deny "the signing key's fingerprint does not match" @(
            "expected  $FINGERPRINT",
            "found     $shown",
            "",
            "This is the check that anchors every other one, so nothing further",
            "was attempted. Either the key you fetched is not VeilVoice's, or the",
            "fingerprint in this script has been altered. Compare it against the",
            "one published in README.md and on the website before going further."
        )
    }
    Write-Good "fingerprint matches $FINGERPRINT"

    # -----------------------------------------------------------------------
    # 3. The signature over the hash list
    # -----------------------------------------------------------------------
    Write-Step "Verifying the signature over SHA256SUMS"
    $verified = Invoke-Gpg $gpg @(
        "--homedir", $homeArg, "--batch", "--verify",
        (ConvertTo-GpgPath "$WORK\SHA256SUMS.asc" $msys),
        (ConvertTo-GpgPath "$WORK\SHA256SUMS" $msys)
    )
    if ($verified.Code -ne 0) {
        Deny "the signature over SHA256SUMS is not valid" @(
            "The hash list is not the one signed by $FINGERPRINT.",
            "Do not use this download.",
            "",
            ($verified.Output | Out-String).Trim()
        )
    }
    Write-Good "signature is good"

    # -----------------------------------------------------------------------
    # 4. The archive's hash, against the now-trusted list
    # -----------------------------------------------------------------------
    Write-Step "Verifying the archive against SHA256SUMS"
    $want = ""
    foreach ($line in (Get-Content "$WORK\SHA256SUMS")) {
        # `hash  name`, and sha256sum writes a `*` before the name in binary mode.
        $parts = $line -split '\s+', 2
        if ($parts.Count -eq 2) {
            $name = $parts[1].Trim().TrimStart('*')
            if ($name -eq $ARCHIVE) { $want = $parts[0].Trim(); break }
        }
    }
    if (-not $want) {
        Deny "$ARCHIVE is not listed in SHA256SUMS" @(
            "The signature was good, so the list is genuine -- it simply does not",
            "mention this file. That means this archive was not part of this release."
        )
    }

    $got = (Get-FileHash -Algorithm SHA256 -Path "$WORK\$ARCHIVE").Hash.ToLower()
    if ($got -ne $want.ToLower()) {
        Deny "the archive's SHA-256 does not match the signed hash list" @(
            "expected  $want",
            "found     $got",
            "",
            "The download does not match what was signed. It is corrupt,",
            "truncated, or not the file that was published."
        )
    }
    Write-Good "sha256 matches ($got)"

    # -----------------------------------------------------------------------
    # 5. Unpack and install
    # -----------------------------------------------------------------------
    Write-Step "Unpacking"
    Expand-Archive -Path "$WORK\$ARCHIVE" -DestinationPath "$WORK\unpacked" -Force

    $src = Get-ChildItem "$WORK\unpacked" -Directory | Select-Object -First 1
    if (-not $src) { Deny "the archive did not contain the directory expected" }

    if (-not $Prefix) { $Prefix = Join-Path $env:LOCALAPPDATA "Programs\VeilVoice" }
    New-Item -ItemType Directory -Path $Prefix -Force | Out-Null
    Copy-Item -Path (Join-Path $src.FullName "*") -Destination $Prefix -Recurse -Force
    Write-Good "installed to $Prefix"

    # 6. Have the installed program check every file, one more way. The archive
    # verified against the signed list, so the binary is trustworthy; now use it
    # to run the full per-file check through its own independent code path. If
    # it disagrees, remove what was installed and stop.
    $vv = Join-Path $Prefix "veilvoice.exe"
    if ($HaveContents -and (Test-Path $vv)) {
        Write-Step "Verifying every file with the installed program"
        & $vv verify auto "$WORK" *> $null
        # Exit 2 means a check ran and failed; 3 means it could not complete,
        # which is not a disagreement. Only 2 undoes the install.
        if ($LASTEXITCODE -eq 0) {
            Write-Good "every file checks out"
        } elseif ($LASTEXITCODE -eq 2) {
            Remove-Item -Path (Join-Path $Prefix "veilvoice.exe") -ErrorAction SilentlyContinue
            Remove-Item -Path (Join-Path $Prefix "veilvoice-gui.exe") -ErrorAction SilentlyContinue
            Deny "the installed program's own check disagreed with the download" @(
                "The archive matched the signed list, but the per-file check",
                "failed. The installed programs have been removed. Do not use this copy."
            )
        } else {
            Write-Say "  the extra per-file check could not complete; the archive"
            Write-Say "  itself already verified against the signed list, so this is fine."
        }
    }

    # On the *user's* PATH, not the machine's: this needs no administrator and
    # affects nobody else's account.
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($null -eq $userPath) { $userPath = "" }
    if (($userPath -split ';') -notcontains $Prefix) {
        [Environment]::SetEnvironmentVariable("Path", ($userPath.TrimEnd(';') + ";" + $Prefix), "User")
        Write-Good "added to your PATH (open a new terminal to pick it up)"
    }

    # -----------------------------------------------------------------------
    # 6. Optional extras -- asked once, defaulting to no
    # -----------------------------------------------------------------------
    if (-not $Yes) {
        Write-Say ""
        Write-Say "  Optional, and nothing depends on them:"
        Write-Say ""
        Write-Say "    VB-CABLE  -- a virtual audio cable, which is what lets the live"
        Write-Say "                 mode feed a veiled microphone into a call. It is"
        Write-Say "                 PROPRIETARY donationware by VB-Audio, not free"
        Write-Say "                 software, and is not bundled with VeilVoice."
        Write-Say "                 Installing it means accepting their licence."
        Write-Say ""
        Write-Say "    Audacity  -- a free audio editor, useful for recording and for"
        Write-Say "                 trimming a file before veiling it. Not bundled: it"
        Write-Say "                 is GPL-2.0-or-later, which cannot be combined with"
        Write-Say "                 this project's GPL-3.0-or-later."
        Write-Say ""
        if (Ask "Open the VB-CABLE download page in your browser?") { $WithVBCable = $true }
        if (Ask "Install Audacity?") { $WithAudacity = $true }
    }

    if ($WithVBCable) {
        # Deliberately not a silent download-and-run. VB-CABLE is proprietary
        # software with its own licence and its own installer, and this script
        # has no business accepting somebody else's terms on your behalf --
        # still less running an unverified third-party installer as part of a
        # script whose entire subject is verifying what you run.
        Write-Step "Opening the VB-CABLE page"
        Start-Process "https://vb-audio.com/Cable/"
        Write-Say "  Follow their instructions, then reboot before using live mode."
    }

    if ($WithAudacity) {
        Write-Step "Installing Audacity"
        if (Test-Have "winget") {
            winget install --id Audacity.Audacity -e --accept-package-agreements --accept-source-agreements
        } else {
            Write-Say "  winget is not available. Get it from https://www.audacityteam.org/"
        }
    }

    # -----------------------------------------------------------------------
    Write-Say ""
    Write-Host "Installed." -ForegroundColor Green
    Write-Say ""
    Write-Say "  Every check passed: the key's fingerprint, the signature over the"
    Write-Say "  hash list, and the archive's hash against that list."
    Write-Say ""
    Write-Say "  Open a new terminal, then:"
    Write-Say "    veilvoice --help"
    Write-Say "    veilvoice info        # what this build supports"
    Write-Say ""
    Write-Say "  The desktop app is veilvoice-gui.exe in:"
    Write-Say "    $Prefix"
    Write-Say ""
    Write-Say "  What it does and does not do:"
    Write-Say "    https://github.com/$REPO/blob/main/docs/WHITEPAPER.md"
    Write-Say ""
}
finally {
    if (Test-Path $WORK) { Remove-Item -Recurse -Force $WORK -ErrorAction SilentlyContinue }
}

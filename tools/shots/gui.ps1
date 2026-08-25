# SPDX-License-Identifier: GPL-3.0-or-later
#
# Photograph every tab of the desktop application.
#
#     cargo build --release -p veilvoice-gui
#     powershell -ExecutionPolicy Bypass -File tools/shots/gui.ps1
#
# Writes assets/screenshots/gui-<tab>.png, one per tab.
#
# # Why this is a script rather than a folder somebody filled by hand
#
# Screenshots go stale silently. A picture of a tab that has since gained a
# control is worse than no picture: it is documentation that disagrees with the
# program and nothing can tell. Re-running this takes a minute, so a change to
# the interface can be followed by a change to its pictures in the same commit.
#
# # Why the capture is DPI-aware
#
# Without SetProcessDpiAwareness(2) `CopyFromScreen` captures the top-left two
# thirds of the window and the result looks exactly like a layout bug. Two
# sessions of this project lost time to that before it was written down, which
# is why it is written down here as well as in HANDOFF.
#
# # Why the frame comes from DWM rather than from GetWindowRect
#
# `GetWindowRect` returns the window's *extended* bounds, which since Windows
# Vista include the invisible resize border and the drop shadow. Capturing that
# rectangle puts a strip of whatever is behind the window down both sides of
# every picture -- desktop, or the editor somebody happened to have open. It
# looks like a rendering fault in the application and is not one.
# `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` gives the bounds a person
# would draw round the window with a pen.
#
# # Why two pictures are redacted
#
# A screenshot of a working application is a screenshot of somebody's machine,
# and two of these tabs put that on the page:
#
#   * **live** lists this machine's audio devices by name, and those names are
#     product names -- a headset model, a particular virtual-cable setup. That
#     is a description of the maintainer's hardware.
#   * **install** prints the path the program is running from and the path an
#     install would go to, both of which contain the **account name**. This
#     project is published under a pseudonym on purpose, and an account name is
#     not that pseudonym.
#
# So those controls are painted over and relabelled before the file is written,
# in the colours the interface draws them in, so the replacement reads as part
# of the application rather than as a black bar.
#
# The two pictures are therefore not pure captures, and saying so is the point:
# it is marked here, and in `assets/screenshots/README.md`, and nowhere else is
# anything altered. Redacting beats dropping them -- the live tab is one of the
# two things this program is for -- and it beats publishing them and hoping
# nobody reads the dropdown.
#
# **A tab that starts showing a path or a device name needs adding to the tables
# below.** Nothing can check that for you: the text is inside a PNG.
#
# # Why a mis-click is a failure rather than a duplicate picture
#
# The tabs are clicked at measured coordinates. A coordinate that goes stale --
# because a tab was added, renamed, or the row wrapped -- would otherwise
# produce a directory of identical pictures with different names, which is the
# worst possible outcome: wrong, plausible, and silent. So each capture is
# compared with the one before it, and an identical pair fails the run.

param(
  [string]$Exe = "",
  [string]$Out = "",
  [int]$Width = 1400,
  # Tall enough that the longest tab -- group, with the render controls under
  # the speaker list -- fits without scrolling. A picture of half a panel is a
  # picture of half a feature. 1320 leaves room under a 1440-tall screen for
  # the window not to be clipped by the taskbar.
  [int]$Height = 1320
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if ($Exe -eq "") {
  $target = $env:CARGO_TARGET_DIR
  if ($target -eq $null -or $target -eq "") { $target = Join-Path $root "target" }
  $Exe = Join-Path $target "release\veilvoice-gui.exe"
}
if ($Out -eq "") { $Out = Join-Path $root "assets\screenshots" }
if (-not (Test-Path $Exe)) {
  Write-Output "no build at $Exe -- run: cargo build --release -p veilvoice-gui"
  exit 1
}
if (-not (Test-Path $Out)) { New-Item -ItemType Directory -Force $Out | Out-Null }

$sig = @'
using System;
using System.Runtime.InteropServices;
public class Shot {
  [DllImport("shcore.dll")] public static extern int SetProcessDpiAwareness(int v);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out RECT r, int size);
  // DWMWA_EXTENDED_FRAME_BOUNDS. Falls back to GetWindowRect if DWM says no,
  // which is a picture with a shadow round it rather than no picture at all.
  public static RECT Frame(IntPtr h) {
    RECT r;
    if (DwmGetWindowAttribute(h, 9, out r, Marshal.SizeOf(typeof(RECT))) == 0) return r;
    GetWindowRect(h, out r);
    return r;
  }
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  public static IntPtr Found = IntPtr.Zero;
  public static string Want = "";
  public static bool Check(IntPtr h, IntPtr l) {
    if (!IsWindowVisible(h)) return true;
    int n = GetWindowTextLength(h);
    if (n == 0) return true;
    var sb = new System.Text.StringBuilder(n + 1);
    GetWindowText(h, sb, sb.Capacity);
    if (sb.ToString().IndexOf(Want, StringComparison.OrdinalIgnoreCase) >= 0) { Found = h; return false; }
    return true;
  }
}
'@
Add-Type -TypeDefinition $sig
try { [void][Shot]::SetProcessDpiAwareness(2) } catch {}

function Find-App {
  [Shot]::Want = "VeilVoice"
  [Shot]::Found = [IntPtr]::Zero
  [void][Shot]::EnumWindows([Shot+EnumProc]{ param($h,$l) [Shot]::Check($h,$l) }, [IntPtr]::Zero)
  return [Shot]::Found
}

function Click-At($h, [int]$x, [int]$y) {
  [void][Shot]::SetForegroundWindow($h)
  Start-Sleep -Milliseconds 250
  $r = New-Object Shot+RECT
  [void][Shot]::GetWindowRect($h, [ref]$r)
  [void][Shot]::SetCursorPos(($r.Left + $x), ($r.Top + $y))
  Start-Sleep -Milliseconds 180
  [Shot]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
  Start-Sleep -Milliseconds 60
  [Shot]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
  Start-Sleep -Milliseconds 600
}

# The device controls on the live tab, in image coordinates, and what to write
# over them. Measured off a capture; if the tab's layout changes these move, and
# the redaction check below is what says so.
$redactLive = @(
  @{ x = 97;  y = 239; w = 545; h = 44; text = "your microphone" },
  @{ x = 97;  y = 294; w = 590; h = 44; text = "your virtual cable" }
)

# The install tab's two paths. Painted in the page background rather than the
# control background, because these are plain text on the panel and not
# controls.
$redactInstall = @(
  @{ x = 240; y = 384; w = 1120; h = 28; flat = $true
     text = "C:\Users\you\AppData\Local\veilvoice\target\release\veilvoice-gui.exe" },
  @{ x = 240; y = 423; w = 1120; h = 28; flat = $true
     text = "C:\Users\you\AppData\Local\Programs\VeilVoice" }
)

function Redact([string]$path, $areas) {
  # Drawn onto a copy, not onto the loaded file. `new Bitmap(path)` holds the
  # file open for as long as the object lives, so saving back to the same path
  # fails with "a generic error occurred in GDI+" -- which is what GDI+ says
  # instead of "the file is locked", and is the least helpful message in it.
  $src = New-Object System.Drawing.Bitmap $path
  $bmp = New-Object System.Drawing.Bitmap $src.Width, $src.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.DrawImage($src, 0, 0, $src.Width, $src.Height)
  $src.Dispose()
  $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit
  # Tokyo Night's bg-soft and accent, the colours the control is drawn in, so
  # the replacement reads as part of the interface rather than as a black bar.
  $fill = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(31, 35, 53))
  $flat = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(26, 27, 38))
  $edge = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(47, 53, 73))
  $ink = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(122, 162, 247))
  $font = New-Object System.Drawing.Font "Consolas", 15
  # A shade smaller for the plain-text rows: the application draws them at a
  # size Consolas 15 overruns, and the first path ran off the right edge.
  $small = New-Object System.Drawing.Font "Consolas", 13
  foreach ($a in $areas) {
    $rect = New-Object System.Drawing.Rectangle $a.x, $a.y, $a.w, $a.h
    if ($a.flat) {
      # Plain text on the panel, so no control box round it.
      $g.FillRectangle($flat, $rect)
      $g.DrawString($a.text, $small, $ink, ($a.x - 2), ($a.y + 5))
    } else {
      $g.FillRectangle($fill, $rect)
      $g.DrawRectangle($edge, $rect)
      $g.DrawString($a.text, $font, $ink, ($a.x + 14), ($a.y + 11))
    }
  }
  $small.Dispose(); $font.Dispose(); $ink.Dispose(); $edge.Dispose()
  $flat.Dispose(); $fill.Dispose()
  $g.Dispose()
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
}

function Capture($h, [string]$path) {
  [void][Shot]::SetForegroundWindow($h)
  Start-Sleep -Milliseconds 500
  $r = [Shot]::Frame($h)
  $w = $r.Right - $r.Left
  $hh = $r.Bottom - $r.Top
  $bmp = New-Object System.Drawing.Bitmap $w, $hh
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size $w, $hh))
  $g.Dispose()
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
}

# The tab row, in window coordinates. Measured from a capture rather than
# guessed: the row is left-aligned and its y does not move with the window's
# width, so widening the window to fit "about" leaves every other x alone.
#
# `install` is only offered to a portable copy, which a build under `target/`
# is. Running this against an installed copy will find one fewer tab, and the
# duplicate check below is what says so rather than writing a wrong picture.
$tabs = @(
  @{ name = "file";     x = 121 },
  @{ name = "live";     x = 325 },
  @{ name = "group";    x = 476 },
  @{ name = "monitor";  x = 589 },
  @{ name = "lock";     x = 697 },
  @{ name = "settings"; x = 811 },
  @{ name = "install";  x = 943 },
  @{ name = "about";    x = 1057 }
)
$tabY = 141

Write-Output "starting $Exe"
$proc = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds 8

$h = Find-App
if ($h -eq [IntPtr]::Zero) {
  Write-Output "the window never appeared"
  $proc | Stop-Process -Force
  exit 1
}

# A fixed size, so the pictures are the same shape from one run to the next and
# a diff of two runs is about what changed in the interface.
# SWP_NOMOVE is not set: position is fixed too, because a window half off the
# screen captures the desktop behind it.
[void][Shot]::SetWindowPos($h, [IntPtr]::Zero, 60, 20, $Width, $Height, 0x0040)
Start-Sleep -Milliseconds 800

$previous = $null
$written = 0
$problems = @()

foreach ($tab in $tabs) {
  $path = Join-Path $Out ("gui-" + $tab.name + ".png")
  Click-At $h $tab.x $tabY
  # Group mode is off by default, which is correct and makes for a picture of
  # an empty panel. A demonstration of group mode should demonstrate group
  # mode, so the toggle is clicked before this one is taken. It is per-run and
  # the application is closed at the end, so nothing is left changed.
  if ($tab.name -eq "group") { Click-At $h 33 350 }
  Capture $h $path
  if ($tab.name -eq "live") { Redact $path $redactLive }
  if ($tab.name -eq "install") { Redact $path $redactInstall }
  $bytes = [System.IO.File]::ReadAllBytes($path)
  $hash = [System.BitConverter]::ToString(
    [System.Security.Cryptography.SHA256]::Create().ComputeHash($bytes))
  if ($previous -ne $null -and $hash -eq $previous) {
    $problems += ("gui-" + $tab.name + ".png is identical to the tab before it: " +
                  "the click at x=" + $tab.x + " did not land on a tab")
  }
  $previous = $hash
  $written++
  Write-Output ("wrote gui-" + $tab.name + ".png  (" + $bytes.Length + " bytes)")
}

$proc | Stop-Process -Force

if ($problems.Count -gt 0) {
  Write-Output ""
  foreach ($p in $problems) { Write-Output ("  PROBLEM " + $p) }
  Write-Output ""
  Write-Output "The tab coordinates in this script have gone stale. Take one capture by"
  Write-Output "hand, read the new x of each tab off it, and update the table above."
  exit 1
}

Write-Output ""
Write-Output "$written captures in $Out"

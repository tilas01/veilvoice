# SPDX-License-Identifier: GPL-3.0-or-later
#
# Photograph every tab of the desktop application.
#
#     powershell -ExecutionPolicy Bypass -File tools/shots/gui.ps1
#
# Why this looks the way it does
# ------------------------------
#
# Three earlier versions of this script drove the interface by clicking, and
# each failed differently:
#
# 1. **Hard-coded tab coordinates.** They went stale the first time a tab was
#    inserted. Every click still landed on *a* tab, so every capture differed
#    and nothing noticed; three tabs were published under the wrong names.
# 2. **Finding the tabs by scanning for lit columns.** Better, and it caught
#    its own failure loudly, but it depends on the gaps between labels being
#    wider than the gaps inside them. Capitalising the labels closed the space
#    between the first two and the scan merged them into one.
# 3. **Clicking at all.** Synthetic mouse input needs the window in the
#    foreground, and Windows refuses to give the foreground to a process that
#    does not already hold it. `SetForegroundWindow` reports that refusal by
#    returning false, which nothing was reading, so the click went nowhere and
#    whichever tab was already open got photographed under nine names.
#
# So this does not click. `veilvoice-gui --tab <name>` opens the window on a
# tab, and the application is started once per tab. There are no coordinates,
# no scanning, no focus and no input.
#
# Capture is `PrintWindow` with PW_RENDERFULLCONTENT, which asks the window to
# draw itself into a bitmap. It needs neither focus nor visibility, so nothing
# in front of it matters -- which is the other half of the same problem, and
# the reason the earlier versions quietly photographed the desktop wallpaper.
#
# The window is sized rather than maximised: see the note beside SetWindowPos.

param(
  [string]$Exe = "$env:CARGO_TARGET_DIR\release\veilvoice-gui.exe",
  [string]$Out = "assets\screenshots"
)

Add-Type -AssemblyName System.Drawing

Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class Shot {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int t, uint flags);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out RECT r, int size);
  [DllImport("shcore.dll")] public static extern int SetProcessDpiAwareness(int v);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }

  public static uint Want = 0;
  public static IntPtr Found = IntPtr.Zero;
  public static bool Check(IntPtr h, IntPtr l) {
    if (!IsWindowVisible(h)) return true;
    uint pid; GetWindowThreadProcessId(h, out pid);
    if (pid != Want) return true;
    StringBuilder sb = new StringBuilder(300); GetWindowText(h, sb, 300);
    if (sb.ToString() == "VeilVoice") { Found = h; return false; }
    return true;
  }

  // The bounds a person sees. GetWindowRect includes the invisible resize
  // border and the drop shadow, which put a strip of desktop down two edges of
  // every capture until this was used instead.
  public static RECT Frame(IntPtr h) {
    RECT r;
    if (DwmGetWindowAttribute(h, 9, out r, Marshal.SizeOf(typeof(RECT))) == 0) return r;
    return new RECT();
  }
}
"@

# Without this, CopyFromScreen and the window rectangles disagree on any display
# that is not at 100%, and every capture is cropped to the top-left corner.
try { [Shot]::SetProcessDpiAwareness(2) | Out-Null } catch {}

if (-not $env:CARGO_TARGET_DIR) {
  $Exe = "target\release\veilvoice-gui.exe"
}
if (-not (Test-Path $Exe)) {
  Write-Error "no build at $Exe -- run: cargo build --release -p veilvoice-gui"
  exit 1
}
New-Item -ItemType Directory -Force $Out | Out-Null

# The tabs, by the names the application answers to. These are
# `Tab::key` in crates/veilvoice-gui/src/app.rs, and a test in that file keeps
# them unique and stable, because each one is also a file name the README links.
$tabs = @("file", "live", "group", "monitor", "lock", "verify", "settings", "install", "about")

# Group mode is off by default, which is correct and makes for a picture of an
# empty panel. It is turned on for these captures through the application's own
# preference, set before the window opens and put back afterwards.
$settings = Join-Path $env:APPDATA "veilvoice\settings.conf"
$saved = $null
if (Test-Path $settings) { $saved = Get-Content -Raw $settings }
$forced = if ($saved) {
  if ($saved -match "always_group\s*=") {
    $saved -replace "always_group\s*=.*", "always_group = true"
  } else {
    $saved.TrimEnd() + "`nalways_group = true`n"
  }
} else {
  "configured = true`nalways_group = true`n"
}
New-Item -ItemType Directory -Force (Split-Path $settings) | Out-Null
Set-Content -Path $settings -Value $forced -Encoding utf8

$PW_RENDERFULLCONTENT = 2
$problems = @()
$prints = @{}

foreach ($tab in $tabs) {
  $proc = Start-Process -FilePath $Exe -ArgumentList "--tab", $tab -PassThru

  # Wait for this process's own window, by process id rather than by title
  # alone: another VeilVoice left open would otherwise be photographed instead.
  [Shot]::Want = [uint32]$proc.Id
  [Shot]::Found = [IntPtr]::Zero
  $h = [IntPtr]::Zero
  for ($i = 0; $i -lt 60 -and $h -eq [IntPtr]::Zero; $i++) {
    Start-Sleep -Milliseconds 400
    [Shot]::Found = [IntPtr]::Zero
    [void][Shot]::EnumWindows([Shot+EnumProc]{ param($a, $b) [Shot]::Check($a, $b) }, [IntPtr]::Zero)
    $h = [Shot]::Found
  }
  if ($h -eq [IntPtr]::Zero) {
    $problems += "$tab : the window never appeared"
    $proc | Stop-Process -Force -ErrorAction SilentlyContinue
    continue
  }

  # A fitted size rather than full screen.
  #
  # Maximising on a 4K display gave 3840x2088 captures whose text was
  # unreadable at any size a page shows them, with wide empty margins down
  # either side where the layout had nothing to put. This is the size the
  # window opens at, widened so all nine tab labels fit and made taller so that
  # every control on the longest tab is visible without scrolling. The picture
  # should show somebody the whole of a panel, which is the only reason to take
  # one.
  [void][Shot]::SetWindowPos($h, [IntPtr]::Zero, 40, 40, 1400, 1000, 0x0004)
  # Long enough for the layout to settle and the first frames to be drawn.
  Start-Sleep -Milliseconds 2500

  $r = [Shot]::Frame($h)
  $w = $r.Right - $r.Left
  $hh = $r.Bottom - $r.Top
  if ($w -le 100 -or $hh -le 100) {
    $problems += "$tab : the window measured ${w}x${hh}"
    $proc | Stop-Process -Force -ErrorAction SilentlyContinue
    continue
  }

  $bmp = New-Object System.Drawing.Bitmap $w, $hh
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $dc = $g.GetHdc()
  $drew = [Shot]::PrintWindow($h, $dc, $PW_RENDERFULLCONTENT)
  $g.ReleaseHdc($dc)
  $g.Dispose()

  if (-not $drew) {
    $problems += "$tab : PrintWindow refused"
    $bmp.Dispose()
    $proc | Stop-Process -Force -ErrorAction SilentlyContinue
    continue
  }

  $path = Join-Path $Out "gui-$tab.png"
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)

  # A cheap fingerprint, so two tabs coming out identical is caught rather than
  # published. That is the failure the coordinate versions of this script kept
  # producing, and it is invisible in a directory listing.
  $sb = New-Object System.Text.StringBuilder
  for ($y = 60; $y -lt [Math]::Min(600, $bmp.Height); $y += 17) {
    for ($x = 20; $x -lt [Math]::Min(900, $bmp.Width); $x += 19) {
      $c = $bmp.GetPixel($x, $y)
      [void]$sb.Append(($c.R -band 0xF0)); [void]$sb.Append(($c.B -band 0xF0))
    }
  }
  $print = $sb.ToString()
  if ($prints.ContainsKey($print)) {
    $problems += "$tab : identical to $($prints[$print]) -- the tab did not change"
  } else {
    $prints[$print] = $tab
  }

  $bmp.Dispose()
  Write-Output ("wrote gui-{0}.png  ({1}x{2})" -f $tab, $w, $hh)
  $proc | Stop-Process -Force -ErrorAction SilentlyContinue
  Start-Sleep -Milliseconds 300
}

# The reader's own settings back, exactly as they were. A screenshot script that
# leaves a preference changed is one that edits somebody's configuration to take
# a picture.
if ($null -ne $saved) {
  Set-Content -Path $settings -Value $saved -Encoding utf8
} elseif (Test-Path $settings) {
  Remove-Item $settings -Force
}

if ($problems.Count -gt 0) {
  Write-Output ""
  foreach ($p in $problems) { Write-Output "  PROBLEM $p" }
  exit 1
}
Write-Output ""
Write-Output ("{0} captures in {1}" -f $tabs.Count, (Resolve-Path $Out))

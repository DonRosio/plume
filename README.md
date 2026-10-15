# Plume

A lightweight native window for **Discord's official web app**, on Windows. Written in Rust, it
displays https://discord.com in **WebView2**, the Chromium engine built into Windows, instead of
bundling its own browser the way Discord desktop (Electron) does.

Plume is not affiliated with Discord. The page is never modified: no injected script, no mod.

The application itself is in French (its menus and messages); this documentation is in English.

## Expected gains (estimates, to be confirmed by your own measurements)

| | Discord desktop | Plume |
|---|---|---|
| Disk space | ~1 GB | a single `.exe` of a few MB (WebView2 is already part of Windows) |
| Everyday memory | ~1 GB, more over long sessions | ~350 to 500 MB |
| Minimised window | — | WebView2 switches to "low memory" mode |
| Start-up | loading screen + updates | immediate |

Most of the remaining memory is used by Discord's web app itself.

### What Plume does to use as little as possible

- **Hidden or minimised window**: the page is no longer drawn (no CPU or GPU wasted) and WebView2
  switches to low-memory mode. Messages, notifications and calls keep working.
- **Unneeded components disabled**: the PDF viewer and Edge's internal UI.
- **Optional eco mode**: Chromium's "low-end device" mode (smaller caches, cheaper rendering, slightly
  lower visual quality at times). Tick it in the tray icon's menu (see below).
- **Never at the expense of security**: site isolation, the sandbox and SmartScreen stay on (a test
  enforces it).

### What you can tune in Discord itself (often the biggest gains)

In Discord's user settings:
- **Accessibility**: reduce motion, and turn off automatic GIF playback, animated emoji and
  animated stickers.
- **Text & Images**: turn off link previews if you do not need them.
- Leave the very busy servers you no longer read: every open server costs memory.

## What works, and the limits

Messages, voice and video calls, screen sharing, notifications, drag-and-drop of files.

Like Discord, **the close button does not quit**: Plume hides in the notification area (near the
clock) and keeps receiving messages, notifications and calls. A left click on the icon reopens the
window; right click → "Quitter" (Quit) to really close it. To keep the icon always visible: Windows
Settings → Personalization → Taskbar → Other system tray icons → turn Plume on.

**Tray icon menu (right click)**: "Ouvrir Plume" (Open), **"Mode éco"** (Eco mode), **"Mode
éphémère"** (Ephemeral mode), "Redémarrer Plume pour appliquer" (Restart to apply), "Quitter" (Quit).
Both modes are remembered; they take effect at the next start of Plume, never in the middle of a call
(the restart is offered, not forced). The `--eco` and `--ephemeral` command-line options still work
and force the mode.

**Mentions badge**: like Discord, a red badge with the number of unread mentions appears on the
taskbar icon and on the notification-area icon. Plume only reads the page's title, where Discord
already shows that number.

**Robustness**: if the page's engine crashes, the page is reloaded; if the whole of WebView2 stops,
Plume restarts. Without a network at launch, Plume retries by itself (5 s, 10 s, 20 s, 40 s, then every
minute). A page that stops responding is deliberately *not* reloaded: during a game it may simply be
starved of CPU, and reloading it would cut the call.

**Window**: size, position and maximised state are remembered (if the screen was unplugged, Plume
goes back to the default position).

**For gaming**: while you play, the game has the focus, so Discord's push-to-talk does not react. Use
**voice activity detection** instead (Discord settings → Voice & Video → Input Mode). With the
window hidden or minimised, Plume draws nothing: the game gets its CPU and GPU back, and the call
continues.

Limits (they are those of Discord in a browser):
- **push-to-talk** and shortcuts only work while the window is active;
- no game detection, no overlay;
- account links (Spotify, Twitch…) and payments open in your browser.

## Security

| Risk | Protection |
|---|---|
| A booby-trapped page calls code on your PC | No page ↔ system bridge: no IPC, no host objects, no web messages |
| Phishing in a window without an address bar | Only `https://discord.com` (exact domain) is displayed in Plume; any other site opens in your browser, where the address is visible |
| A pop-up that replaces Discord (fake OAuth consent page…) | Pop-ups never open in Plume, only in the browser (at most one per second) |
| Launching a program through a link (`ms-msdt:`, `search-ms:`, `file:`…) | Only `http` and `https` are passed to the browser, everything else is blocked |
| Microphone, camera… access by embedded content | Microphone, camera, notifications and autoplay are granted **only** to `https://discord.com`; everything else (geolocation, clipboard reading, files…) is denied; decisions are never remembered |
| Booby-trapped download | Only files from Discord's servers are accepted, and SmartScreen checks them |
| Theft of saved credentials | Password saving and autofill are disabled; developer tools are disabled |
| Booby-trapped DLL next to the executable | The DLLs Plume depends on are only looked up in `System32`, never in the current folder or in the `PATH` |
| Web engine flaw | WebView2 is maintained and updated automatically by Microsoft, often faster than an Electron application's frozen Chromium |
| Supply chain | 78 crates, versions pinned (`Cargo.lock`), audited with `cargo audit`; you build it yourself |

**Your data** is in `%LOCALAPPDATA%\Plume`, readable only by your Windows account. Deleting that
folder logs you out completely.

**Unavoidable limit:** as with Discord desktop or a browser, malware already running under your Windows
session can read the saved session. If you want nothing to be stealable, run Plume in **ephemeral
mode**: nothing is written to disk, and you log in again at every launch.

```powershell
.\plume.exe --ephemeral
```

**Recommendations:** turn on two-factor authentication on your Discord account (ideally with a
passkey), keep Windows up to date, and keep `plume.exe` in its own folder.

To report a vulnerability in Plume, see [SECURITY.md](SECURITY.md).

## Getting `plume.exe` without installing anything (GitHub Actions)

The `.github/workflows/build.yml` file has GitHub's Windows servers compile Plume. Your PC then needs
neither Rust nor the Visual Studio tools (several GB).

1. Fork this repository (or push a copy to a private repository of your own).
2. On every push, the **Actions** tab checks the code (format, lint, tests, dependency audit, secret
   scan over the whole history) and builds; `plume.exe` and its SHA-256 are downloadable under
   "Artifacts". Every Monday, the audit and the secret scan also run without a push: a vulnerability
   found in the meantime in a dependency makes the run fail, and GitHub e-mails you.
3. For a release: create a `v…` tag (for example `v0.2.0`). A **Release** is published with
   `plume.exe` and its SHA-256.

Check the downloaded executable by comparing its hash with the `plume.exe.sha256` file published next
to it (on a public repository, GitHub also adds a build-provenance attestation, which you can verify
with `gh attestation verify plume.exe --repo DonRosio/plume`):

```powershell
(Get-FileHash plume.exe -Algorithm SHA256).Hash
```

Windows will show a SmartScreen warning once (downloaded, unsigned file): "More info" → "Run anyway".
The GitHub actions used are pinned to an exact commit hash, so that a hijacked tag cannot change
what runs.

## Building on Windows

Prerequisites:
1. Windows 10 or 11 with the **WebView2 Runtime** (already present on Windows 11; otherwise install
   the "Evergreen Runtime" from Microsoft's website).
2. **Rust**: download `rustup-init.exe` from https://rustup.rs and run it. If it offers, accept the
   installation of the Visual Studio C++ tools (needed for linking). Keep the default toolchain
   (`msvc`): it produces a standalone `.exe`.

Then, in a terminal opened in the `plume` folder:

```powershell
cargo build --release
```

The executable is `target\release\plume.exe`, with the feather icon built in. Build from a personal
folder (for example `Documents\plume`): Cargo must be able to write to `target\`.

Installation: copy **only** `plume.exe` to `C:\Program Files\Plume\` (administrator rights are asked
once; this folder cannot be modified by an ordinary program, which protects the executable). Launch
it, then right click its taskbar icon → "Pin to taskbar".

Optional, but advised before each build: check the dependencies.

```powershell
cargo install cargo-audit
```

```powershell
cargo audit
```

To contribute: a hook refuses the commit if a secret is detected or if the code is not formatted.
It needs [gitleaks](https://github.com/gitleaks/gitleaks#installing) and is enabled once per clone with
`git config core.hooksPath .githooks`.

## Test checklist

1. **Start-up**: dark background, then Discord; log in with password + 2FA, QR code or passkey.
2. **Relaunch**: you are still logged in; a second launch brings back the existing window.
3. **Files**: send an attachment; download one (the download bar appears).
4. **Voice call**: you hear and are heard.
5. **Screen sharing.**
6. **Notifications** while the window is minimised.
7. **Links**: a link to another site opens in your browser, not in Plume.
8. **Ephemeral mode**: with `--ephemeral`, after closing and relaunching, Discord asks you to log in
   again.

## Measuring memory (PowerShell)

Do it after about ten minutes of comparable use (same servers open).

Discord desktop:

```powershell
"{0:N0} MB" -f ((Get-Process Discord -ErrorAction SilentlyContinue | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB)
```

Plume (the `plume.exe` process and its WebView2 processes):

```powershell
$ids = @((Get-Process plume).Id) + @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object { $_.CommandLine -like '*\Plume\*' } | ForEach-Object ProcessId); "{0:N0} MB" -f ((Get-Process -Id $ids | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB)
```

## Updates

WebView2 updates with Windows, and Discord's web app at every load. For Plume itself, get the new
GitHub Release (or rebuild after `cargo audit`). There is deliberately no automatic update, hence no
update channel to hijack.

## Approach

This project started from a concrete need: reduce the memory Discord uses on my gaming PC, while I
play League of Legends in voice chat with my friends.

I designed and drove it, and developed it with the assistance of **Claude** (Anthropic), used as a
pair-programming partner. The commits carry a `Co-Authored-By: Claude` trailer for that reason. My
role:

- **Define the need and the requirements**: a lighter alternative to Discord desktop, usable with my
  existing account, and as secure as possible.
- **Decide the architecture.** I first explored a fully native client, ruled out because it would
  have broken Discord's terms of service and put my account at risk. I then compared several routes
  (browser, an existing third-party client, a WebView2 window) before choosing WebView2 for Windows,
  which keeps voice calls.
- **Set the trade-offs**: security never sacrificed to performance, an optional eco mode rather than
  an imposed one, no automatic start with Windows, no automatic reload of a page that stops
  responding (so as not to cut a call in the middle of a game).
- **Validate on my Windows machine**: build, behaviour, memory measurements.

The code was written with Claude, then checked: unit tests of the security rules, strict static
analysis (clippy), dependency audit, automatic build and publication on GitHub. The main flaws found
during reviews were fixed: pop-ups able to replace the window, SmartScreen disabled by default in the
wry library, booby-trapped DLLs.

## Code

| File | Role |
|---|---|
| `src/policy.rs` | All the security rules (navigation, pop-ups, permissions, downloads), unit-tested |
| `src/app.rs` | Window, WebView2 hardening, event handlers, tray icon menu |
| `src/badge.rs` | Reading the mention count from the title, and drawing the badge |
| `src/settings.rs` | Settings and window position (`%LOCALAPPDATA%\Plume\settings.txt`) |
| `.github/workflows/build.yml` | Automatic build and checks on GitHub |
| `src/main.rs` | Entry point |
| `build.rs`, `assets/plume.ico` | Icon and program name embedded in `plume.exe` |

Tests (on any system):

```bash
cargo test
```

Checking the Windows build from Linux:

```bash
cargo clippy --target x86_64-pc-windows-msvc -- -D warnings
```

## License

[MIT](LICENSE).

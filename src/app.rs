//! Windows shell: window, WebView2 hardening, permission handler, event loop.
//!
//! Security model (see README): the page is untrusted remote content. It gets no
//! native bridge (no IPC, no host objects, no web messages), no devtools, no
//! password/autofill storage, only the permissions a voice/video chat needs and only
//! for https://discord.com, and it can never navigate the window away from Discord.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tao::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::platform::windows::WindowExtWindows;
use tao::window::{Icon, Window, WindowBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_PERMISSION_KIND, COREWEBVIEW2_PERMISSION_KIND_AUTOPLAY,
    COREWEBVIEW2_PERMISSION_KIND_CAMERA, COREWEBVIEW2_PERMISSION_KIND_MICROPHONE,
    COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS, COREWEBVIEW2_PERMISSION_STATE_ALLOW,
    COREWEBVIEW2_PERMISSION_STATE_DENY, COREWEBVIEW2_PROCESS_FAILED_KIND,
    COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED,
    COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED, COREWEBVIEW2_WEB_ERROR_STATUS,
    COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED, ICoreWebView2, ICoreWebView2_4,
    ICoreWebView2DownloadStartingEventArgs, ICoreWebView2NavigationCompletedEventArgs,
    ICoreWebView2PermissionRequestedEventArgs, ICoreWebView2PermissionRequestedEventArgs3,
    ICoreWebView2ProcessFailedEventArgs, ICoreWebView2Settings4,
};
use webview2_com::{
    DownloadStartingEventHandler, NavigationCompletedEventHandler, PermissionRequestedEventHandler,
    ProcessFailedEventHandler, take_pwstr,
};
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::LibraryLoader::{
    LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, SetDefaultDllDirectories,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, MB_ICONERROR, MB_OK, MessageBoxW, SW_RESTORE, SW_SHOW, SW_SHOWNORMAL,
    SetForegroundWindow, ShowWindow,
};
use windows::core::{BOOL, HSTRING, Interface, PCWSTR, PWSTR, w};
use wry::{
    MemoryUsageLevel, NewWindowResponse, WebContext, WebView, WebViewBuilder,
    WebViewBuilderExtWindows, WebViewExtWindows,
};

use crate::badge;
use crate::policy::{self, ExternalOpenThrottle, Navigation, Permission};
use crate::settings::{Geometry, Settings};

const TITLE: &str = "Plume";
/// Dark background shown before Discord paints (no white flash).
const BACKGROUND: (u8, u8, u8, u8) = (49, 51, 56, 255);

/// Requests coming from WebView2 callbacks, handled on the event loop.
enum UserEvent {
    Load(String),
    OpenInBrowser(String),
    /// Tray icon clicked or "Ouvrir Plume" chosen.
    ShowWindow,
    /// "Quitter" chosen in the tray menu: the only way to really exit.
    Quit,
    /// Any tray menu entry (handled on the event loop, where the menu items live).
    Menu(MenuId),
    /// The page title changed (it carries Discord's unread-mention count).
    Title(String),
    /// The page process died: reload it.
    Reload,
    /// The WebView2 browser process died: restart Plume entirely.
    Restart,
    /// Loading Discord failed (offline at start-up…): retry with back-off.
    LoadFailed,
    LoadSucceeded,
    /// Back-off delay elapsed after a failed load.
    Retry,
}

pub fn run() {
    // DLLs loaded at runtime are looked up in the application folder and System32 only:
    // the current directory and %PATH% are never searched (DLL planting).
    // SAFETY: plain Win32 call, made before any other library is loaded on purpose.
    unsafe {
        let _ = SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();

    let Some(instance) = SingleInstance::acquire() else {
        focus_existing_window();
        return;
    };
    let Some(data_dir) = data_dir() else {
        fatal("Dossier %LOCALAPPDATA% introuvable.");
    };
    let settings_path = data_dir.join("settings.txt");
    let mut settings = Settings::load(&settings_path);
    // Command-line switches force a mode on; otherwise the tray menu choices apply.
    let eco = settings.eco || args.iter().any(|arg| arg == "--eco");
    let ephemeral = settings.ephemeral || args.iter().any(|arg| arg == "--ephemeral");

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let mut builder = WindowBuilder::new()
        .with_title(TITLE)
        .with_inner_size(LogicalSize::new(1200.0, 800.0))
        .with_min_inner_size(LogicalSize::new(800.0, 500.0))
        .with_background_color(BACKGROUND)
        .with_window_icon(Icon::from_rgba(icon_rgba(), ICON_SIZE, ICON_SIZE).ok());
    // Restore the last position only if it is still on a connected screen.
    if let Some(g) = settings.window.filter(|g| {
        event_loop.available_monitors().any(|m| {
            let (p, s) = (m.position(), m.size());
            let (cx, cy) = (g.x + 100, g.y + 50);
            cx >= p.x && cy >= p.y && cx < p.x + s.width as i32 && cy < p.y + s.height as i32
        })
    }) {
        builder = builder
            .with_position(PhysicalPosition::new(g.x, g.y))
            .with_inner_size(PhysicalSize::new(g.width, g.height))
            .with_maximized(g.maximized);
    }
    let window = match builder.build(&event_loop) {
        Ok(window) => window,
        Err(err) => fatal(&format!("Impossible de créer la fenêtre : {err}")),
    };

    // Per-user profile under %LOCALAPPDATA%\Plume (ACL: current user only, like any app data).
    // In ephemeral mode WebView2 runs InPrivate: nothing of the session is written to disk.
    let mut context = WebContext::new(Some(data_dir));
    let navigation_proxy = event_loop.create_proxy();
    let popup_proxy = event_loop.create_proxy();
    let title_proxy = event_loop.create_proxy();

    let webview = WebViewBuilder::new_with_web_context(&mut context)
        .with_incognito(ephemeral)
        // Replaces wry's defaults, which would turn SmartScreen off.
        .with_additional_browser_args(policy::browser_arguments(eco))
        .with_devtools(false)
        .with_clipboard(false) // no clipboard *reading* by scripts; Ctrl+C / Ctrl+V still work
        .with_general_autofill_enabled(false)
        .with_hotkeys_zoom(true)
        .with_back_forward_navigation_gestures(false)
        .with_background_color(BACKGROUND)
        .with_browser_extensions_enabled(false)
        .with_navigation_handler(move |url| on_navigation(&navigation_proxy, &url))
        .with_new_window_req_handler(move |url, _features| {
            if let Some(target) = policy::popup(&url) {
                let _ = popup_proxy.send_event(UserEvent::OpenInBrowser(target));
            }
            NewWindowResponse::Deny
        })
        .with_document_title_changed_handler(move |title| {
            let _ = title_proxy.send_event(UserEvent::Title(title));
        })
        .build(&window);
    let webview = match webview {
        Ok(webview) => webview,
        Err(err) => fatal(&format!(
            "Impossible de démarrer WebView2 : {err}\n\nInstallez le « Runtime WebView2 Evergreen » depuis le site de Microsoft (developer.microsoft.com/microsoft-edge/webview2), puis relancez Plume."
        )),
    };

    // Lock everything down *before* the first navigation: some settings only apply to
    // documents loaded after they are set.
    if let Err(err) = harden(&webview, event_loop.create_proxy()) {
        fatal(&format!("Impossible de sécuriser WebView2 : {err}"));
    }
    if let Err(err) = webview.load_url(policy::START_URL) {
        fatal(&format!("Impossible d'ouvrir Discord : {err}"));
    }

    // Like Discord: the close button hides the window, Plume keeps running (notifications,
    // calls) and lives in the notification area until "Quitter".
    let tray = match Tray::build(event_loop.create_proxy(), eco, ephemeral) {
        Ok(tray) => tray,
        Err(err) => fatal(&format!(
            "Impossible de créer l'icône de notification : {err}"
        )),
    };

    let proxy = event_loop.create_proxy();
    let mut instance = Some(instance);
    let mut throttle = ExternalOpenThrottle::default();
    let mut hidden = false;
    let mut mentions = 0;
    let mut failed_loads = 0u32;

    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::Load(url)) => {
                let _ = webview.load_url(&url);
            }
            Event::UserEvent(UserEvent::OpenInBrowser(url)) => {
                if throttle.allow(Instant::now()) {
                    open_in_browser(&url);
                }
            }
            Event::UserEvent(UserEvent::ShowWindow) => show(&window, &webview, &mut hidden),
            Event::UserEvent(UserEvent::Quit) => {
                remember_geometry(&window, &mut settings);
                let _ = settings.save(&settings_path);
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::Menu(id)) => {
                if id == tray.open {
                    show(&window, &webview, &mut hidden);
                } else if id == tray.quit {
                    let _ = proxy.send_event(UserEvent::Quit);
                } else if id == tray.restart {
                    let _ = proxy.send_event(UserEvent::Restart);
                } else if id == *tray.eco.id() || id == *tray.ephemeral.id() {
                    settings.eco = tray.eco.is_checked();
                    settings.ephemeral = tray.ephemeral.is_checked();
                    let _ = settings.save(&settings_path);
                    // These modes are fixed when WebView2 starts: offer a restart (never forced,
                    // it would cut an ongoing call).
                    let pending =
                        tray.eco.is_checked() != eco || tray.ephemeral.is_checked() != ephemeral;
                    tray.restart_item.set_enabled(pending);
                }
            }
            Event::UserEvent(UserEvent::Title(title)) => {
                let count = badge::mentions_from_title(&title);
                if count != mentions {
                    mentions = count;
                    tray.show_mentions(count);
                    let overlay = (count > 0)
                        .then(|| Icon::from_rgba(badge::overlay(count, 16), 16, 16).ok())
                        .flatten();
                    window.set_overlay_icon(overlay.as_ref());
                }
            }
            Event::UserEvent(UserEvent::Reload) => {
                let _ = webview.reload();
            }
            Event::UserEvent(UserEvent::Restart) => {
                remember_geometry(&window, &mut settings);
                let _ = settings.save(&settings_path);
                drop(instance.take()); // let the new process take the single-instance lock
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe).args(&args).spawn();
                }
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::LoadFailed) => {
                // 5 s, 10 s, 20 s, 40 s, then every minute: cheap while offline.
                let delay = Duration::from_secs((5u64 << failed_loads.min(4)).min(60));
                failed_loads = failed_loads.saturating_add(1);
                let retry = proxy.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(delay);
                    let _ = retry.send_event(UserEvent::Retry);
                });
            }
            Event::UserEvent(UserEvent::LoadSucceeded) => failed_loads = 0,
            Event::UserEvent(UserEvent::Retry) => {
                let _ = webview.load_url(policy::START_URL);
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                remember_geometry(&window, &mut settings);
                let _ = settings.save(&settings_path);
                window.set_visible(false);
                set_hidden(&webview, true, &mut hidden);
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_) | WindowEvent::Moved(_),
                ..
            } => {
                remember_geometry(&window, &mut settings);
                set_hidden(
                    &webview,
                    window.is_minimized() || !window.is_visible(),
                    &mut hidden,
                );
            }
            _ => {}
        }
    });
}

fn show(window: &Window, webview: &WebView, hidden: &mut bool) {
    window.set_visible(true);
    window.set_minimized(false);
    window.set_focus();
    set_hidden(webview, false, hidden);
}

/// Out of sight: stop painting the page (no CPU/GPU spent on frames nobody sees; scripts,
/// notifications and calls keep running) and let WebView2 trim its memory.
fn set_hidden(webview: &WebView, hidden_now: bool, hidden: &mut bool) {
    if hidden_now != *hidden {
        *hidden = hidden_now;
        let level = if hidden_now {
            MemoryUsageLevel::Low
        } else {
            MemoryUsageLevel::Normal
        };
        let _ = webview.set_visible(!hidden_now);
        let _ = webview.set_memory_usage_level(level);
    }
}

/// Keeps the last "normal" placement (a minimised window reports meaningless coordinates).
fn remember_geometry(window: &Window, settings: &mut Settings) {
    if !window.is_visible() || window.is_minimized() {
        return;
    }
    let maximized = window.is_maximized();
    let previous = settings.window;
    let geometry = if maximized {
        previous.map(|g| Geometry {
            maximized: true,
            ..g
        })
    } else {
        window.outer_position().ok().map(|p| {
            let size = window.inner_size();
            Geometry {
                x: p.x,
                y: p.y,
                width: size.width,
                height: size.height,
                maximized: false,
            }
        })
    };
    if geometry.is_some() {
        settings.window = geometry;
    }
}

/// Notification-area icon: left click reopens the window, right click shows the menu.
struct Tray {
    icon: TrayIcon,
    eco: CheckMenuItem,
    ephemeral: CheckMenuItem,
    restart_item: MenuItem,
    open: MenuId,
    quit: MenuId,
    restart: MenuId,
}

impl Tray {
    fn build(proxy: EventLoopProxy<UserEvent>, eco: bool, ephemeral: bool) -> Result<Self, String> {
        let open = MenuItem::new("Ouvrir Plume", true, None);
        let eco_item = CheckMenuItem::new("Mode éco (moins de mémoire)", true, eco, None);
        let ephemeral_item =
            CheckMenuItem::new("Mode éphémère (rien sur le disque)", true, ephemeral, None);
        let restart_item = MenuItem::new("Redémarrer Plume pour appliquer", false, None);
        let quit = MenuItem::new("Quitter", true, None);
        let menu = Menu::with_items(&[
            &open,
            &PredefinedMenuItem::separator(),
            &eco_item,
            &ephemeral_item,
            &restart_item,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .map_err(|e| e.to_string())?;

        let menu_proxy = proxy.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let _ = menu_proxy.send_event(UserEvent::Menu(event.id));
        }));
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = proxy.send_event(UserEvent::ShowWindow);
            }
        }));

        let icon = tray_icon::Icon::from_rgba(icon_rgba(), ICON_SIZE, ICON_SIZE)
            .map_err(|e| e.to_string())?;
        let icon = TrayIconBuilder::new()
            .with_tooltip(TITLE)
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            icon,
            open: open.id().clone(),
            quit: quit.id().clone(),
            restart: restart_item.id().clone(),
            eco: eco_item,
            ephemeral: ephemeral_item,
            restart_item,
        })
    }

    fn show_mentions(&self, count: u32) {
        let rgba = if count > 0 {
            badge::on_icon(&icon_rgba(), ICON_SIZE, count)
        } else {
            icon_rgba()
        };
        if let Ok(icon) = tray_icon::Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE) {
            let _ = self.icon.set_icon(Some(icon));
        }
        let tooltip = match count {
            0 => TITLE.to_owned(),
            1 => format!("{TITLE} — 1 mention non lue"),
            n => format!("{TITLE} — {n} mentions non lues"),
        };
        let _ = self.icon.set_tooltip(Some(tooltip));
    }
}

fn on_navigation(proxy: &EventLoopProxy<UserEvent>, url: &str) -> bool {
    match policy::classify(url) {
        Navigation::Stay => true,
        Navigation::Redirect(target) => {
            let _ = proxy.send_event(UserEvent::Load(target));
            false
        }
        Navigation::OpenInBrowser(target) => {
            let _ = proxy.send_event(UserEvent::OpenInBrowser(target));
            false
        }
        Navigation::Block => false,
    }
}

/// WebView2 settings wry does not expose, plus our own permission and download handlers
/// (wry's do not tell which page is asking, and would hide WebView2's download UI).
fn harden(webview: &WebView, proxy: EventLoopProxy<UserEvent>) -> windows::core::Result<()> {
    let core: ICoreWebView2 = webview.webview();
    let crash_proxy = proxy.clone();
    let crashes = ProcessFailedEventHandler::create(Box::new(move |_sender, args| {
        on_process_failed(&crash_proxy, args)
    }));
    let navigations = NavigationCompletedEventHandler::create(Box::new(move |_sender, args| {
        on_navigation_completed(&proxy, args)
    }));
    let permissions = PermissionRequestedEventHandler::create(Box::new(on_permission_requested));
    let downloads = DownloadStartingEventHandler::create(Box::new(on_download_starting));
    let mut token = 0i64;
    // SAFETY: COM calls on a live ICoreWebView2 owned by `webview`, made on the UI thread
    // that created it, as WebView2 requires.
    unsafe {
        let settings = core.Settings()?;
        settings.SetIsWebMessageEnabled(false)?; // no window.chrome.webview.postMessage
        settings.SetAreHostObjectsAllowed(false)?; // no native objects reachable from scripts
        settings.SetAreDevToolsEnabled(false)?;
        if let Ok(settings4) = settings.cast::<ICoreWebView2Settings4>() {
            settings4.SetIsPasswordAutosaveEnabled(false)?;
            settings4.SetIsGeneralAutofillEnabled(false)?;
        }
        core.add_PermissionRequested(&permissions, &mut token)?;
        core.add_ProcessFailed(&crashes, &mut token)?;
        core.add_NavigationCompleted(&navigations, &mut token)?;
        core.cast::<ICoreWebView2_4>()?
            .add_DownloadStarting(&downloads, &mut token)?;
    }
    Ok(())
}

/// A dead page process is reloaded, a dead WebView2 browser process restarts Plume. An
/// *unresponsive* page is left alone on purpose: under a heavy game it may just be starved
/// of CPU, and reloading it would cut the voice call.
fn on_process_failed(
    proxy: &EventLoopProxy<UserEvent>,
    args: Option<ICoreWebView2ProcessFailedEventArgs>,
) -> windows::core::Result<()> {
    let Some(args) = args else { return Ok(()) };
    let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
    // SAFETY: `args` is a valid event-args object for the duration of the callback.
    unsafe { args.ProcessFailedKind(&mut kind)? };
    let request = match kind {
        COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED => UserEvent::Reload,
        COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED => UserEvent::Restart,
        // GPU, utility… processes are restarted by WebView2 itself.
        _ => return Ok(()),
    };
    let _ = proxy.send_event(request);
    Ok(())
}

/// Detects failed loads of Discord (e.g. no network when Plume starts) so they are retried.
fn on_navigation_completed(
    proxy: &EventLoopProxy<UserEvent>,
    args: Option<ICoreWebView2NavigationCompletedEventArgs>,
) -> windows::core::Result<()> {
    let Some(args) = args else { return Ok(()) };
    let mut success = BOOL::default();
    let mut status = COREWEBVIEW2_WEB_ERROR_STATUS::default();
    // SAFETY: `args` is a valid event-args object for the duration of the callback.
    unsafe {
        args.IsSuccess(&mut success)?;
        args.WebErrorStatus(&mut status)?;
    }
    let request = if success.as_bool() {
        UserEvent::LoadSucceeded
    } else if status == COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED {
        return Ok(()); // our own policy blocked a navigation: not a failure
    } else {
        UserEvent::LoadFailed
    };
    let _ = proxy.send_event(request);
    Ok(())
}

/// Attachments from Discord's CDNs download normally (with WebView2's usual progress UI and
/// SmartScreen checks); anything else is cancelled before a single byte is written.
fn on_download_starting(
    _sender: Option<ICoreWebView2>,
    args: Option<ICoreWebView2DownloadStartingEventArgs>,
) -> windows::core::Result<()> {
    let Some(args) = args else { return Ok(()) };
    // SAFETY: `args` is a valid event-args object for the duration of the callback, which
    // WebView2 invokes on the UI thread.
    unsafe {
        let mut uri = PWSTR::null();
        args.DownloadOperation()?.Uri(&mut uri)?;
        if !policy::download_allowed(&take_pwstr(uri)) {
            args.SetCancel(true)?;
        }
    }
    Ok(())
}

/// Called by WebView2 for every permission request, from the page or from any frame in it.
fn on_permission_requested(
    _sender: Option<ICoreWebView2>,
    args: Option<ICoreWebView2PermissionRequestedEventArgs>,
) -> windows::core::Result<()> {
    let Some(args) = args else { return Ok(()) };
    // SAFETY: `args` is a valid event-args object for the duration of the callback, which
    // WebView2 invokes on the UI thread.
    unsafe {
        let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
        args.PermissionKind(&mut kind)?;
        let mut uri = PWSTR::null();
        args.Uri(&mut uri)?;
        let uri = take_pwstr(uri);

        let allowed = policy::permission_allowed(&uri, permission_from(kind));
        // Do not remember decisions in the profile: our policy is asked every time.
        if let Ok(args3) = args.cast::<ICoreWebView2PermissionRequestedEventArgs3>() {
            args3.SetSavesInProfile(false)?;
        }
        args.SetState(if allowed {
            COREWEBVIEW2_PERMISSION_STATE_ALLOW
        } else {
            COREWEBVIEW2_PERMISSION_STATE_DENY
        })
    }
}

fn permission_from(kind: COREWEBVIEW2_PERMISSION_KIND) -> Permission {
    match kind {
        COREWEBVIEW2_PERMISSION_KIND_MICROPHONE => Permission::Microphone,
        COREWEBVIEW2_PERMISSION_KIND_CAMERA => Permission::Camera,
        COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS => Permission::Notifications,
        COREWEBVIEW2_PERMISSION_KIND_AUTOPLAY => Permission::Autoplay,
        _ => Permission::Other,
    }
}

/// Hands an http(s) URL to the default browser. `policy::classify` already restricted the
/// scheme; checking again here keeps this function safe on its own (no other protocol
/// handler can ever be launched from Plume).
fn open_in_browser(url: &str) {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return;
    }
    // SAFETY: plain Win32 call with valid, NUL-terminated wide strings.
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &HSTRING::from(url),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

fn data_dir() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(local).join("Plume"))
}

/// One Plume per Windows session: a second launch brings the first window to the front.
struct SingleInstance(HANDLE);

impl SingleInstance {
    fn acquire() -> Option<Self> {
        // SAFETY: plain Win32 calls; the handle is closed in `Drop` (or by Windows at exit).
        unsafe {
            let handle = CreateMutexW(None, true, w!("Local\\Plume.SingleInstance")).ok()?;
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(handle);
                return None;
            }
            Some(Self(handle))
        }
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        // SAFETY: the handle was returned by CreateMutexW and is closed exactly once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn focus_existing_window() {
    // SAFETY: plain Win32 calls; a missing window is simply ignored.
    unsafe {
        if let Ok(hwnd) = FindWindowW(PCWSTR::null(), &HSTRING::from(TITLE)) {
            let _ = ShowWindow(hwnd, SW_SHOW); // the window may be hidden in the tray
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

fn fatal(message: &str) -> ! {
    // SAFETY: plain Win32 call with valid, NUL-terminated wide strings.
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(message),
            &HSTRING::from(TITLE),
            MB_OK | MB_ICONERROR,
        );
    }
    std::process::exit(1)
}

const ICON_SIZE: u32 = 64;

/// 64×64 RGBA icon drawn in code (no image decoder, no asset file): a white quill on a
/// rounded violet square. Same drawing as `assets/plume.ico`.
fn icon_rgba() -> Vec<u8> {
    const SIZE: u32 = ICON_SIZE;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Rounded square (radius 14).
            let (cx, cy) = (fx.clamp(14.0, 50.0), fy.clamp(14.0, 50.0));
            let inside_square = (fx - cx).powi(2) + (fy - cy).powi(2) <= 14.0f32.powi(2);
            // Quill: an ellipse along the diagonal from bottom-left to top-right.
            let (u, v) = (
                (fx - 32.0 + fy - 32.0) / 2f32.sqrt(),
                (fx - 32.0 - (fy - 32.0)) / 2f32.sqrt(),
            );
            let vane = (v / 26.0).powi(2) + ((u - 2.0) / 7.0).powi(2) <= 1.0;
            let shaft = u.abs() <= 1.2 && v > -30.0 && v < 26.0;
            let pixel = match (inside_square, vane, shaft) {
                (false, ..) => [0, 0, 0, 0],
                (true, true, true) => [80, 64, 200, 255], // rachis drawn through the vane
                (true, true, false) | (true, false, true) => [255, 255, 255, 255],
                (true, false, false) => [108, 92, 231, 255],
            };
            rgba.extend_from_slice(&pixel);
        }
    }
    rgba
}

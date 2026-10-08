//! Security policy of Plume, kept free of any platform code so that it is unit-tested
//! on every OS. Everything the page asks for goes through one of these functions:
//!
//! - top-level navigations → [`classify`], pop-ups → [`popup`]
//! - permission requests (microphone, camera…) → [`permission_allowed`]
//! - downloads → [`download_allowed`]
//! - links opened in the user's browser → [`ExternalOpenThrottle`]

use std::time::{Duration, Instant};

use url::Url;

/// The only site allowed to run inside the window.
pub const APP_ORIGIN: &str = "https://discord.com";
pub const START_URL: &str = "https://discord.com/app";

/// Hosts serving attachments; downloads from anywhere else are refused.
const DOWNLOAD_ORIGINS: [&str; 3] = [
    APP_ORIGIN,
    "https://cdn.discordapp.com",
    "https://media.discordapp.net",
];

/// Longest URL we accept to hand over to the system browser.
const MAX_EXTERNAL_URL_LEN: usize = 4096;

/// Command-line switches for the WebView2 browser process.
///
/// - Edge's PDF viewer and web-based internal UI are not needed by Discord: fewer components
///   loaded.
/// - `eco` (opt-in, `plume.exe --eco`): Chromium's low-end-device mode, smaller caches and
///   cheaper rendering at the cost of some visual quality.
///
/// Never disables SmartScreen, site isolation or the sandbox: savings must not cost security.
pub fn browser_arguments(eco: bool) -> String {
    let mut args = String::from("--disable-features=msWebOOUI,msPdfOOUI");
    if eco {
        args.push_str(" --enable-low-end-device-mode");
    }
    args
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Navigation {
    /// The official Discord web app: load it in the window.
    Stay,
    /// A Discord alias (`discord.gg`, `discordapp.com`…): load this canonical URL instead.
    Redirect(String),
    /// Any other web page: hand it to the default browser (http/https only).
    OpenInBrowser(String),
    /// Anything else (other schemes such as `discord:`, `file:`, `javascript:`, `ms-*:`,
    /// malformed URLs…): do nothing.
    Block,
}

/// Decides what to do with a URL the page wants to navigate to (or open in a new window).
pub fn classify(raw: &str) -> Navigation {
    let Ok(url) = Url::parse(raw) else {
        return Navigation::Block;
    };
    // Credentials in URLs are a phishing trick (`https://discord.com@evil.example`): never follow them.
    if !url.username().is_empty() || url.password().is_some() {
        return Navigation::Block;
    }
    match url.scheme() {
        "https" => {}
        "http" => return open_in_browser(&url),
        _ => return Navigation::Block,
    }
    if is_app_origin(&url) {
        return Navigation::Stay;
    }
    match url.host_str() {
        Some("discord.gg") => invite_redirect(&url),
        Some("www.discord.com" | "discordapp.com" | "www.discordapp.com")
            if url.port().is_none() =>
        {
            let mut canonical = url.clone();
            if canonical.set_host(Some("discord.com")).is_err() {
                return Navigation::Block;
            }
            Navigation::Redirect(canonical.into())
        }
        _ => open_in_browser(&url),
    }
}

/// Pop-ups (`window.open`, `target="_blank"`) never load inside Plume: an embedded third-party
/// frame must not be able to replace the Discord window (e.g. with an OAuth consent page).
/// Returns the http(s) URL to hand to the default browser, if any.
pub fn popup(raw: &str) -> Option<String> {
    match classify(raw) {
        Navigation::Stay => Url::parse(raw).ok().map(String::from),
        Navigation::Redirect(target) | Navigation::OpenInBrowser(target) => Some(target),
        Navigation::Block => None,
    }
}

/// `https://discord.gg/<code>` → `https://discord.com/invite/<code>`.
fn invite_redirect(url: &Url) -> Navigation {
    let code = url.path().trim_matches('/');
    let valid = !code.is_empty()
        && code.len() <= 64
        && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if valid {
        Navigation::Redirect(format!("{APP_ORIGIN}/invite/{code}"))
    } else {
        open_in_browser(url)
    }
}

fn open_in_browser(url: &Url) -> Navigation {
    let serialized: String = url.as_str().to_owned(); // normalised and percent-encoded by the parser
    if serialized.len() > MAX_EXTERNAL_URL_LEN {
        return Navigation::Block;
    }
    Navigation::OpenInBrowser(serialized)
}

/// Exactly `https://discord.com` (default port, no credentials). Sub-domains and look-alikes
/// such as `discord.com.evil.example` do not match.
fn is_app_origin(url: &Url) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some("discord.com")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Microphone,
    Camera,
    Notifications,
    Autoplay,
    /// Geolocation, clipboard reading, file system, MIDI, sensors, window management…
    Other,
}

/// Only the page loaded from discord.com gets what a voice/video chat needs. Everything
/// else, and every request from embedded third-party frames, is refused.
pub fn permission_allowed(requesting_uri: &str, permission: Permission) -> bool {
    let from_app = Url::parse(requesting_uri).is_ok_and(|url| is_app_origin(&url));
    from_app
        && matches!(
            permission,
            Permission::Microphone
                | Permission::Camera
                | Permission::Notifications
                | Permission::Autoplay
        )
}

/// Attachments come from Discord's CDNs (or `blob:` URLs created by discord.com itself).
pub fn download_allowed(raw: &str) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    let origin = url.origin();
    DOWNLOAD_ORIGINS
        .iter()
        .filter_map(|allowed| Url::parse(allowed).ok())
        .any(|allowed| allowed.origin() == origin)
}

/// At most one link handed to the browser per second, so that a misbehaving page cannot
/// flood the desktop with browser windows.
#[derive(Debug, Default)]
pub struct ExternalOpenThrottle {
    last: Option<Instant>,
}

impl ExternalOpenThrottle {
    pub const MIN_INTERVAL: Duration = Duration::from_secs(1);

    pub fn allow(&mut self, now: Instant) -> bool {
        if self
            .last
            .is_some_and(|last| now.duration_since(last) < Self::MIN_INTERVAL)
        {
            return false;
        }
        self.last = Some(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discord_stays_in_the_window() {
        assert_eq!(classify("https://discord.com/app"), Navigation::Stay);
        assert_eq!(
            classify("https://discord.com/channels/1/2"),
            Navigation::Stay
        );
        assert_eq!(
            classify("https://DISCORD.com/login?redirect_to=%2Fapp"),
            Navigation::Stay
        );
    }

    #[test]
    fn look_alikes_never_stay() {
        for url in [
            "https://discord.com.evil.example/login",
            "https://evil.example/discord.com",
            "https://discord.co/login",
            "https://xn--dscord-6ya.com/",
            "https://canary.discord.com/app",
            "https://discord.com:8443/app",
        ] {
            assert!(
                matches!(classify(url), Navigation::OpenInBrowser(_)),
                "{url}"
            );
        }
        assert_eq!(
            classify("http://discord.com/app"),
            Navigation::OpenInBrowser("http://discord.com/app".into())
        );
    }

    #[test]
    fn credentials_and_dangerous_schemes_are_blocked() {
        for url in [
            "https://discord.com@evil.example/",
            "https://user:pass@discord.com/app",
            "javascript:alert(1)",
            "file:///C:/Windows/System32/calc.exe",
            "discord://-/channels/@me",
            "ms-msdt:/id PCWDiagnostic",
            "search-ms:query=x",
            "data:text/html,<script>alert(1)</script>",
            "about:blank",
            "not a url",
        ] {
            assert_eq!(classify(url), Navigation::Block, "{url}");
        }
    }

    #[test]
    fn discord_aliases_are_canonicalised() {
        assert_eq!(
            classify("https://discord.gg/rust-fr"),
            Navigation::Redirect("https://discord.com/invite/rust-fr".into())
        );
        assert_eq!(
            classify("https://discordapp.com/channels/1/2?x=1#y"),
            Navigation::Redirect("https://discord.com/channels/1/2?x=1#y".into())
        );
        assert_eq!(
            classify("https://www.discord.com/app"),
            Navigation::Redirect("https://discord.com/app".into())
        );
        // A weird invite path is not trusted: the browser handles it.
        assert!(matches!(
            classify("https://discord.gg/a/b"),
            Navigation::OpenInBrowser(_)
        ));
    }

    #[test]
    fn popups_always_go_to_the_browser() {
        assert_eq!(
            popup("https://discord.com/oauth2/authorize?client_id=1"),
            Some("https://discord.com/oauth2/authorize?client_id=1".into())
        );
        assert_eq!(
            popup("https://discord.gg/rust-fr"),
            Some("https://discord.com/invite/rust-fr".into())
        );
        assert_eq!(
            popup("https://example.com/"),
            Some("https://example.com/".into())
        );
        assert_eq!(popup("about:blank"), None);
        assert_eq!(popup("javascript:alert(1)"), None);
    }

    #[test]
    fn external_links_are_normalised() {
        assert_eq!(
            classify("https://example.com/a b?q=é"),
            Navigation::OpenInBrowser("https://example.com/a%20b?q=%C3%A9".into())
        );
        let huge = format!("https://example.com/{}", "a".repeat(MAX_EXTERNAL_URL_LEN));
        assert_eq!(classify(&huge), Navigation::Block);
    }

    #[test]
    fn permissions_only_for_discord_and_only_what_calls_need() {
        assert!(permission_allowed(
            "https://discord.com/channels/1/2",
            Permission::Microphone
        ));
        assert!(permission_allowed(
            "https://discord.com/",
            Permission::Camera
        ));
        assert!(permission_allowed(
            "https://discord.com/",
            Permission::Notifications
        ));
        assert!(!permission_allowed(
            "https://discord.com/",
            Permission::Other
        ));
        assert!(!permission_allowed(
            "https://www.youtube.com/embed/x",
            Permission::Microphone
        ));
        assert!(!permission_allowed(
            "https://123.discordsays.com/",
            Permission::Camera
        ));
        assert!(!permission_allowed(
            "http://discord.com/",
            Permission::Microphone
        ));
        assert!(!permission_allowed("garbage", Permission::Microphone));
    }

    #[test]
    fn downloads_only_from_discord() {
        assert!(download_allowed(
            "https://cdn.discordapp.com/attachments/1/2/file.png"
        ));
        assert!(download_allowed(
            "https://media.discordapp.net/attachments/1/2/image.png?width=400"
        ));
        assert!(download_allowed(
            "blob:https://discord.com/5b2f1c3e-1111-2222-3333-444455556666"
        ));
        assert!(!download_allowed("https://evil.example/payload.exe"));
        assert!(!download_allowed(
            "http://cdn.discordapp.com/attachments/1/2/file.png"
        ));
        assert!(!download_allowed("blob:https://evil.example/uuid"));
        assert!(!download_allowed(
            "data:application/octet-stream;base64,AAAA"
        ));
    }

    #[test]
    fn browser_arguments_never_weaken_security() {
        for eco in [false, true] {
            let args = browser_arguments(eco).to_lowercase();
            for forbidden in [
                "smartscreen",
                "site-isolation",
                "no-sandbox",
                "disable-web-security",
                "remote-debugging",
            ] {
                assert!(!args.contains(forbidden), "{forbidden} in {args}");
            }
        }
        assert!(browser_arguments(true).contains("--enable-low-end-device-mode"));
        assert!(!browser_arguments(false).contains("low-end"));
    }

    #[test]
    fn browser_opening_is_throttled() {
        let mut throttle = ExternalOpenThrottle::default();
        let t0 = Instant::now();
        assert!(throttle.allow(t0));
        assert!(!throttle.allow(t0 + Duration::from_millis(300)));
        assert!(throttle.allow(t0 + Duration::from_millis(1200)));
    }
}

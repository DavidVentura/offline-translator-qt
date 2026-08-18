use std::ffi::{CStr, c_char, c_void};

pub const URI_SCHEME: &str = "offline-translator://";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchIntent {
    LiveCamera,
}

pub fn parse_launch_uri(uri: &str) -> Option<LaunchIntent> {
    let rest = uri.strip_prefix(URI_SCHEME)?;
    let action = rest
        .split(['?', '#'])
        .next()
        .expect("split always yields one element")
        .trim_end_matches('/');
    match action {
        "camera" => Some(LaunchIntent::LiveCamera),
        _ => None,
    }
}

pub fn intent_from_args(args: &[String]) -> Option<LaunchIntent> {
    args.iter().find_map(|arg| parse_launch_uri(arg))
}

type UriSink = Box<dyn Fn(LaunchIntent) + Send>;

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn translator_install_uri_handler(
        callback: extern "C" fn(*mut c_void, *const c_char),
        context: *mut c_void,
    );
}

extern "C" fn dispatch_uri(context: *mut c_void, uri: *const c_char) {
    let sink = unsafe { &*(context as *const UriSink) };
    let uri = unsafe { CStr::from_ptr(uri) }
        .to_string_lossy()
        .into_owned();
    match parse_launch_uri(&uri) {
        Some(intent) => sink(intent),
        None => eprintln!("uri handler: ignoring unrecognized uri {uri}"),
    }
}

/// Take URIs the shell dispatches to this already-running instance. The sink lives as long as the
/// process, so it is leaked rather than tracked.
#[cfg(target_os = "linux")]
pub fn install(sink: impl Fn(LaunchIntent) + Send + 'static) {
    let sink: UriSink = Box::new(sink);
    let context = Box::into_raw(Box::new(sink)) as *mut c_void;
    unsafe { translator_install_uri_handler(dispatch_uri, context) };
}

// Only Lomiri dispatches URIs to a running process; everywhere else the URI can
// only arrive on the command line.
#[cfg(not(target_os = "linux"))]
pub fn install(_sink: impl Fn(LaunchIntent) + Send + 'static) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_camera_uri() {
        assert_eq!(
            parse_launch_uri("offline-translator://camera"),
            Some(LaunchIntent::LiveCamera)
        );
        assert_eq!(
            parse_launch_uri("offline-translator://camera/"),
            Some(LaunchIntent::LiveCamera)
        );
        assert_eq!(
            parse_launch_uri("offline-translator://camera?foo=1"),
            Some(LaunchIntent::LiveCamera)
        );
    }

    #[test]
    fn rejects_other_uris() {
        assert_eq!(parse_launch_uri("offline-translator://settings"), None);
        assert_eq!(parse_launch_uri("https://example.com/camera"), None);
        assert_eq!(parse_launch_uri("camera"), None);
    }

    #[test]
    fn picks_the_uri_out_of_argv() {
        let args = [
            "/usr/bin/offline-translator-linux".to_string(),
            "offline-translator://camera".to_string(),
        ];
        assert_eq!(intent_from_args(&args), Some(LaunchIntent::LiveCamera));
        assert_eq!(intent_from_args(&args[..1]), None);
    }
}

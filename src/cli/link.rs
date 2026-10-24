//! App launches: a saved host's name or ID, or the links bastion hosts
//! pass the way they open Xshell:
//! `ShellRS ssh://user@host:port`, or `ShellRS -url ssh://… -newtab 名称`;
//! and WinSCP: `ShellRS sftp://user@host:port`, or with
//! `/sessionname=名称` in front.
//!
//! Only told apart from the `shellrs` command's arguments here; the link
//! itself is read by the app (`host::SshLink`), which says what is wrong
//! with it in a notification. Opened again, ShellRS hands the link to the
//! one already running, inside [`super::Request::Activate`].

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::host::standard_url;

/// What an app launch opens. Link launches keep their existing wire shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OpenTarget {
    LocalDirectory { directory: PathBuf },
    Host { host: String },
    Link(OpenLink),
}

impl OpenTarget {
    /// A local file URL delivered by macOS. Only directories are opened;
    /// documents and scripts must never become shell commands.
    pub fn from_directory_url(value: &str) -> Option<Self> {
        let directory = url::Url::parse(value).ok()?.to_file_path().ok()?;
        directory
            .is_dir()
            .then_some(Self::LocalDirectory { directory })
    }
}

/// Recognize an app launch before passing other arguments to the external
/// CLI. `--host` also allows names that collide with CLI commands or URLs.
pub fn launch_arguments(args: &[OsString]) -> Option<OpenTarget> {
    if let [option, directory] = args
        && option == "--working-directory"
    {
        let directory = std::path::absolute(directory).ok()?;
        return Some(OpenTarget::LocalDirectory { directory });
    }
    if let [value] = args {
        if let Some(target) = OpenTarget::from_directory_url(&value.to_string_lossy()) {
            return Some(target);
        }
        let path = PathBuf::from(value);
        if path.is_absolute() && path.is_dir() {
            return Some(OpenTarget::LocalDirectory {
                directory: std::path::absolute(path).ok()?,
            });
        }
    }
    if let [option, host] = args
        && option == "--host"
    {
        return Some(OpenTarget::Host {
            host: host.to_string_lossy().into_owned(),
        });
    }
    if let Some(link) = link_arguments(args) {
        return Some(OpenTarget::Link(link));
    }
    let [host] = args else {
        return None;
    };
    let host = host.to_string_lossy();
    // Derive reserved names from clap so new subcommands keep working.
    use clap::CommandFactory as _;
    if host.starts_with('-')
        || host == "help"
        || super::Cli::command()
            .get_subcommands()
            .any(|command| command.get_name() == host)
    {
        return None;
    }
    Some(OpenTarget::Host {
        host: host.into_owned(),
    })
}

/// A link to connect to, as the caller wrote it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenLink {
    /// May hold a password. Empty when the arguments named no link, so the
    /// app can say so: a second ShellRS on Windows has no console to.
    pub url: String,
    /// What to call the tab: Xshell's `-newtab`, WinSCP's `/sessionname`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<String>,
}

impl fmt::Debug for OpenLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenLink")
            .field("url", &without_password(&self.url))
            .field("tab", &self.tab)
            .finish()
    }
}

/// `url` with its password replaced, for logs, read as the app reads it.
/// Never the text itself when it cannot be read: where the password ends is
/// unknown then.
fn without_password(url: &str) -> String {
    if url.is_empty() {
        return String::new();
    }
    match url::Url::parse(&standard_url(url)) {
        Ok(mut parsed) => {
            if parsed.password().is_some() {
                let _ = parsed.set_password(Some("***"));
            }
            parsed.to_string()
        }
        Err(_) => "<unreadable link>".into(),
    }
}

/// Xshell's options. `-newwin` asks for a window of its own, which means
/// nothing to ShellRS's one window.
const URL_OPTION: &str = "-url";
const TAB_OPTION: &str = "-newtab";
const WINDOW_OPTION: &str = "-newwin";

/// WinSCP's switches, written `/name[=value]` or `-name[=value]`:
/// `/sessionname=名称` names the tab, the others mean nothing to ShellRS
/// (`/newinstance` asks for a window of its own, `/ini=nul` not to read
/// WinSCP's settings, `/privatekey=` and `/hostkey=` are ShellRS's to
/// know).
const WINSCP_SWITCHES: &[&str] = &[
    SESSION_NAME,
    "newinstance",
    "privatekey",
    "hostkey",
    "passphrase",
    "rawsettings",
    "ini",
    "log",
    "timeout",
];
const SESSION_NAME: &str = "sessionname";

/// The link `args` (without the program) open, or `None` when they are the
/// `shellrs` command's. Only the first argument decides, so `shellrs exec
/// web "curl http://x"` stays a command: a link launch starts with one of
/// Xshell's options, one of WinSCP's switches or the link itself.
pub fn link_arguments(args: &[OsString]) -> Option<OpenLink> {
    let args: Vec<String> = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let first = args.first()?;
    if !is_option(first) && winscp_switch(first).is_none() && !looks_like_link(first) {
        return None;
    }
    let mut given = None;
    let mut bare = None;
    let mut tab = None;
    let mut args = args.into_iter().peekable();
    while let Some(arg) = args.next() {
        let option = arg.to_lowercase();
        if option == URL_OPTION {
            let url = args.next();
            given = given.or(url);
        } else if option == TAB_OPTION {
            // Its name, unless the next argument is something else.
            if args
                .peek()
                .is_some_and(|next| !next.starts_with('-') && !looks_like_link(next))
            {
                tab = tab_name(&args.next().unwrap_or_default()).or(tab);
            }
        } else if let Some((switch, value)) = winscp_switch(&arg) {
            if switch == SESSION_NAME {
                tab = value.and_then(tab_name).or(tab);
            }
        } else if option == WINDOW_OPTION || arg.starts_with('-') {
            // Ignored, like any option ShellRS does not know.
        } else if bare.is_none() && looks_like_link(&arg) {
            bare = Some(arg);
        }
    }
    Some(OpenLink {
        url: given.or(bare).unwrap_or_default(),
        tab,
    })
}

fn is_option(arg: &str) -> bool {
    [URL_OPTION, TAB_OPTION, WINDOW_OPTION].contains(&arg.to_lowercase().as_str())
}

/// The name and value of one of WinSCP's switches, the name in lower case.
fn winscp_switch(arg: &str) -> Option<(String, Option<&str>)> {
    let switch = arg.strip_prefix(['/', '-'])?;
    let (name, value) = match switch.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (switch, None),
    };
    let name = name.to_lowercase();
    WINSCP_SWITCHES
        .contains(&name.as_str())
        .then_some((name, value))
}

/// A tab's name as given, without the spaces or quotes around it; `None`
/// when that leaves nothing.
fn tab_name(given: &str) -> Option<String> {
    let name = given.trim().trim_matches('"').trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// `scheme://…`, whatever the scheme: the app says which ones it opens.
fn looks_like_link(arg: &str) -> bool {
    let Some((scheme, _)) = arg.split_once("://") else {
        return false;
    };
    let mut chars = scheme.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(args: &[&str]) -> Option<OpenLink> {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        link_arguments(&args)
    }

    fn open(url: &str, tab: Option<&str>) -> Option<OpenLink> {
        Some(OpenLink {
            url: url.into(),
            tab: tab.map(str::to_string),
        })
    }

    fn launch(args: &[&str]) -> Option<OpenTarget> {
        launch_arguments(&args.iter().map(OsString::from).collect::<Vec<_>>())
    }

    #[test]
    fn local_directories_open_from_paths_arguments_and_macos_urls() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("项目 spaces # ' $()");
        std::fs::create_dir(&directory).unwrap();
        let expected = OpenTarget::LocalDirectory {
            directory: directory.clone(),
        };
        let path = directory.to_str().unwrap();
        let url = url::Url::from_directory_path(&directory).unwrap();
        assert_eq!(launch(&[path]), Some(expected.clone()));
        assert_eq!(
            launch(&["--working-directory", path]),
            Some(expected.clone())
        );
        assert_eq!(launch(&[url.as_str()]), Some(expected.clone()));
        assert_eq!(
            OpenTarget::from_directory_url(url.as_str()),
            Some(expected.clone())
        );
        let wire = serde_json::to_vec(&expected).unwrap();
        assert_eq!(
            serde_json::from_slice::<OpenTarget>(&wire).unwrap(),
            expected
        );

        let file = directory.join("run.command");
        std::fs::write(&file, "exit 1").unwrap();
        for value in [
            url::Url::from_file_path(file).unwrap().to_string(),
            url::Url::from_directory_path(directory.join("missing"))
                .unwrap()
                .to_string(),
            "file://other-machine/tmp/".into(),
            "ssh://root@example.com".into(),
        ] {
            assert_eq!(OpenTarget::from_directory_url(&value), None, "{value}");
        }
    }

    #[test]
    fn a_saved_host_name_or_id_launches_the_app() {
        for host in ["web-01", "生产 服务器", "0123456789abcdef", "", " web "] {
            let expected = Some(OpenTarget::Host { host: host.into() });
            assert_eq!(launch(&[host]), expected);
            assert_eq!(launch(&["--host", host]), expected);
        }
        // Explicit names can look like CLI commands, options or links.
        for host in ["exec", "hosts", "--help", "ssh://root@host"] {
            assert_eq!(
                launch(&["--host", host]),
                Some(OpenTarget::Host { host: host.into() })
            );
        }
    }

    #[test]
    fn saved_host_launches_preserve_cli_commands_and_links() {
        use clap::CommandFactory as _;
        for command in super::super::Cli::command().get_subcommands() {
            assert_eq!(launch(&[command.get_name()]), None);
        }
        for args in [
            &[][..],
            &["help"],
            &["--help"],
            &["--version"],
            &["--host"],
            &["--host", "web", "extra"],
            &["exec", "web", "uname -a"],
            &["hosts", "list"],
            &["web", "extra"],
        ] {
            assert_eq!(launch(args), None, "{args:?}");
        }
        assert_eq!(
            launch(&["ssh://root@host"]),
            open("ssh://root@host", None).map(OpenTarget::Link)
        );
        assert_eq!(
            launch(&["-url", "sftp://root@host", "-newtab", "Files"]),
            open("sftp://root@host", Some("Files")).map(OpenTarget::Link)
        );
    }

    #[test]
    fn launch_targets_round_trip_without_changing_the_link_wire_format() {
        let link = launch(&["ssh://root@host"]).unwrap();
        assert_eq!(
            serde_json::to_string(&link).unwrap(),
            r#"{"url":"ssh://root@host"}"#
        );
        let host = launch(&["生产 服务器"]).unwrap();
        assert_eq!(
            serde_json::to_string(&host).unwrap(),
            r#"{"host":"生产 服务器"}"#
        );
        for target in [host, link] {
            let wire = serde_json::to_vec(&target).unwrap();
            assert_eq!(serde_json::from_slice::<OpenTarget>(&wire).unwrap(), target);
        }
    }

    #[test]
    fn a_link_on_its_own_opens() {
        let url = "ssh://b478e26f-811b-4a90-81c3-74929127898a@172.16.0.28:12024";
        assert_eq!(link(&[url]), open(url, None));
    }

    #[test]
    fn xshells_options_are_understood_in_any_case_and_order() {
        let url = "ssh://deploy:secret@10.0.0.9:2222";
        assert_eq!(
            link(&["-url", url, "-newtab", "生产库"]),
            open(url, Some("生产库"))
        );
        assert_eq!(
            link(&["-NewTab", " 生产库 ", "-newwin", "-URL", url]),
            open(url, Some("生产库"))
        );
        // `-newtab` without a name.
        assert_eq!(link(&["-newtab", url]), open(url, None));
        assert_eq!(link(&["-newwin", url, "-newtab"]), open(url, None));
        // `-url` wins over a bare link, and the first of each counts.
        assert_eq!(
            link(&["ssh://a@b", "-url", url, "-url", "ssh://c@d"]),
            open(url, None)
        );
        // Unknown options are passed over, their values with them unless
        // they look like a link.
        assert_eq!(link(&["-newwin", "-folder", "x", url]), open(url, None));
    }

    #[test]
    fn winscps_switches_are_understood_too() {
        // How JumpServer opens WinSCP: the link alone.
        let url = "sftp://b478e26f:secret@172.16.0.28:12024";
        assert_eq!(link(&[url]), open(url, None));
        assert_eq!(
            link(&["/sessionname=生产库", url]),
            open(url, Some("生产库"))
        );
        assert_eq!(
            link(&[
                "/newinstance",
                "/ini=nul",
                "-SessionName=\"文件 服务器\"",
                url
            ]),
            open(url, Some("文件 服务器"))
        );
        assert_eq!(
            link(&[
                "/rawsettings",
                "Compression=1",
                url,
                "/hostkey=ssh-ed25519 255 x"
            ]),
            open(url, None)
        );
        assert_eq!(link(&["/sessionname=生产库"]), open("", Some("生产库")));
    }

    #[test]
    fn options_without_a_link_still_open_so_the_app_can_say_why() {
        assert_eq!(link(&["-newtab", "生产库"]), open("", Some("生产库")));
        assert_eq!(link(&["-url"]), open("", None));
    }

    #[test]
    fn the_commands_arguments_are_left_to_the_command() {
        for args in [
            &[][..],
            &["list"],
            &["exec", "web", "curl http://example.com"],
            &["upload", "web", "./a", "/tmp/"],
            &["--help"],
            &["-h"],
            &["--version"],
            &["ssh-not-a-link"],
            &["://nothing-before"],
            &["/tmp/sessionname"],
            &["/usr/bin/sftp://x"],
        ] {
            assert_eq!(link(args), None, "{args:?}");
        }
    }

    #[test]
    fn debug_hides_the_password() {
        let shown = format!(
            "{:?}",
            OpenLink {
                url: "ssh://deploy:hunter2@10.0.0.9".into(),
                tab: None,
            }
        );
        assert!(!shown.contains("hunter2"), "{shown}");
        assert!(shown.contains("deploy"), "{shown}");
        let unreadable = format!(
            "{:?}",
            OpenLink {
                url: "ssh://deploy:hunter2@[bad".into(),
                tab: None,
            }
        );
        assert!(!unreadable.contains("hunter2"), "{unreadable}");
        // A user name past the first `/`, as Xshell reads it.
        let slashed = format!(
            "{:?}",
            OpenLink {
                url: "ssh://ops@corp/10.20.0.9/root:hunter2@bastion:8013".into(),
                tab: None,
            }
        );
        assert!(!slashed.contains("hunter2"), "{slashed}");
        assert!(slashed.contains("bastion:8013"), "{slashed}");
    }
}

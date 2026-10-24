// A release build on Windows is a GUI program, so opening it does not bring
// up a console window as well. Debug builds keep theirs for the logs.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use gpui_kit::component::{Root, notification::Notification};
use gpui_kit::*;

use shellrs::host::{HostDatabase, HostStore};
use shellrs::settings::{SettingsProblem, SettingsStore};

fn main() {
    let args = shellrs::cli::command_line_arguments().unwrap_or_default();
    #[cfg(target_os = "macos")]
    let (args, background_launch) = {
        let mut args = args;
        let background = shellrs::app::take_background_launch(&mut args);
        (args, background)
    };
    // A saved host's name or ID opens its terminal; SSH/SFTP links open
    // external connections, including Xshell/WinSCP-style launch options.
    // Recognize these on every build, including Windows GUI releases.
    let target = shellrs::cli::launch_arguments(&args);
    // `shellrs hosts list`, `shellrs exec …`: the command, not the app. Not in a
    // Windows release build, which has nowhere to print: there the command
    // is `shellrs-cli.exe`, which 设置 → 外部 CLI puts on the PATH.
    #[cfg(any(not(windows), debug_assertions))]
    if target.is_none() && !args.is_empty() {
        std::process::exit(shellrs::cli::main(args));
    }
    // One Termirs per data directory. Each keeps the hosts in memory and
    // writes its changes through, so a second one would write over the
    // first one's; opened again, Termirs brings the running one forward
    // and hands it the target to open.
    if shellrs::cli::activate_running_app(&shellrs::app::cli_socket_path(), target.clone()) {
        return;
    }
    #[cfg(target_os = "macos")]
    if target.is_some() && !background_launch {
        if let Err(error) = shellrs::app::launch_in_background(&args) {
            eprintln!("shellrs: cannot start the app in the background: {error}");
            std::process::exit(1);
        }
        return;
    }
    let app = gpui_kit::application().with_assets(shellrs::app::AppAssets);
    // LaunchServices sends file URLs both at startup and to a running app.
    // Queue them before creating the window so cold launches lose nothing.
    let (opened_urls, directory_requests) = async_channel::unbounded();
    app.on_open_urls(move |urls| {
        for target in urls
            .iter()
            .filter_map(|url| shellrs::cli::OpenTarget::from_directory_url(url))
        {
            let _ = opened_urls.try_send(target);
        }
    });
    // macOS: the Dock icon of a Termirs whose window was closed, which only
    // hid it (see `hide_when_closed`).
    app.on_reopen(|cx| {
        if let Some(shellrs::app::MainWindow(window)) = cx.try_global().copied() {
            window
                .update(cx, |_, window, cx| shellrs::app::bring_forward(window, cx))
                .ok();
        }
    });
    app.run(|cx| {
        // Read before anything is built: the settings choose the language
        // every text is in, and 应用 › 窗口 where the window opens.
        let (settings, settings_problem) = open_settings();
        let locale = settings.settings().language.resolved();
        shellrs::i18n::set_locale(locale);
        // Defaults that become the user's own, the example highlight rules
        // a first start fills in, are made while reading: read again, now
        // in the language just chosen.
        let (settings, settings_problem) = if locale == shellrs::i18n::LOCALES[0] {
            (settings, settings_problem)
        } else {
            open_settings()
        };
        shellrs::init(cx);
        // 显示或隐藏 Termirs takes its key from the system; tests never do.
        shellrs::app::listen_for_system_hotkey(cx);
        shellrs::app::show_logo_when_unbundled(cx);
        cx.activate(true);

        let saved = shellrs::workspace::WindowState::load(&shellrs::app::window_state_path());
        let options =
            shellrs::workspace::window_options(saved.as_ref(), settings.settings().window, cx);
        cx.spawn(async move |cx| {
            let window = cx
                .open_window(options, |window, cx| {
                    window.activate_window();
                    window.set_window_title("Termirs");
                    shellrs::app::hide_when_closed(window, cx);
                    // Drawn by the processor (a virtual machine without 3D),
                    // Termirs draws fewer frames.
                    shellrs::app::note_renderer(window, cx);
                    let (store, problem) = open_store();
                    let store = cx.new(|_| store);
                    let settings = cx.new(|_| settings);
                    let target = target.or_else(|| directory_requests.try_recv().ok());
                    let workspace = cx.new(|cx| {
                        shellrs::workspace::Workspace::new(store, settings, target, window, cx)
                    });
                    workspace.update(cx, |_, cx| {
                        cx.spawn_in(window, async move |this, cx| {
                            while let Ok(target) = directory_requests.recv().await {
                                if this
                                    .update_in(cx, |workspace, window, cx| {
                                        shellrs::app::bring_forward(window, cx);
                                        workspace.open_target(target, window, cx);
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                            }
                        })
                        .detach()
                    });
                    // Not pushed here: the window has no `Root` to show it yet.
                    let settings_problem = settings_problem.map(|problem| problem.message());
                    for problem in [problem, settings_problem].into_iter().flatten() {
                        shellrs::workspace::notify_once_open(
                            Notification::error(problem),
                            window,
                            cx,
                        );
                    }
                    cx.new(|cx| {
                        let root = Root::new(workspace, window, cx);
                        #[cfg(target_os = "macos")]
                        let root = root.bg(transparent_black());
                        root
                    })
                })
                .expect("failed to open window");
            cx.update(|cx| cx.set_global(shellrs::app::MainWindow(window.into())));
        })
        .detach();
    });
    // On Windows the event loop ends and `run` returns: an update that is
    // to be installed now starts its setup, which waits for this process.
    shellrs::update::start_handed_over();
}

/// Load the hosts saved on disk. If the database cannot be opened the
/// window still comes up, on a store that forgets everything when it closes;
/// the returned message says so.
///
/// Either way the store gets the system keychain, which is where passwords
/// live. A machine without one still runs; the host form says so and
/// disables its password field. Pasted and generated private keys go
/// beside the database; the credentials follow them when the data folder
/// has moved.
fn open_store() -> (HostStore, Option<SharedString>) {
    let secrets = shellrs::secrets::system_secret_store();
    match load_store() {
        // Keys are kept only with a database to remember them: a store
        // that forgets everything would leave them behind unused.
        Ok(store) => {
            let mut store = store
                .with_secrets(secrets)
                .with_key_dir(shellrs::app::keys_dir())
                .with_terminal_output_dir(shellrs::app::terminal_output_dir());
            let problem = store.adopt_moved_keys();
            (store, problem)
        }
        Err(error) => {
            eprintln!("shellrs: cannot open the local database: {error}");
            (
                HostStore::empty().with_secrets(secrets),
                Some(shellrs::app::database_unavailable(&error)),
            )
        }
    }
}

/// Load the settings file. Without a data directory the settings still
/// work, only for this run; the returned problem says so.
fn open_settings() -> (SettingsStore, Option<SettingsProblem>) {
    match shellrs::app::settings_path() {
        Ok(path) => SettingsStore::load(path),
        Err(error) => (
            SettingsStore::in_memory(),
            Some(SettingsProblem::Unsaved(error.to_string())),
        ),
    }
}

fn load_store() -> anyhow::Result<HostStore> {
    let path = shellrs::app::database_path()?;
    Ok(HostStore::load(HostDatabase::open(&path)?)?)
}

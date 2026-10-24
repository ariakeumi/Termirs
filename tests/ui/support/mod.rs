//! What the tests share: the imports, the seeded ids, opening a workspace
//! and driving it frame by frame.

// Every test module starts with `use crate::support::*;`, which brings in
// these along with the fixtures and fakes.
pub use gpui_kit::component::{ActiveTheme as _, Root, WindowExt as _, notification::Notification};
pub use gpui_kit::test::{TestAppContextExt, TestWindowExt};
pub use gpui_kit::{
    App, AppContext as _, ClipboardItem, ElementId, Entity, InputEvent as _, MouseButton,
    MouseMoveEvent, TestAppContext, WindowHandle, point, px, size,
};
pub use semver::Version;
pub use shellrs::app::{
    CenterTab, CheckForUpdates, ClearTerminal, CloseScope, CloseTabs, CollapseAllGroups,
    ConnectGroup, ConnectHost, CopyCredentialPublicKey, CopyHostAddress, CopyHostId,
    DeleteCredential, DeleteForward, DeleteGroup, DeleteHost, DisconnectTerminal, EditCredential,
    EditForward, EditHost, ExpandAllGroups, FindInTerminal, FindNextInTerminal,
    FindPreviousInTerminal, FocusSearch, InstallCliCommand, NewHostInGroup, NewLocalTerminal,
    OpenExplorer, OpenSettings, ReconnectTerminal, RemoveAgentSkill, RenameGroup, RenameTerminal,
    StartForward, StopForward, ToggleHostPanel, ToggleToolSidebar,
};
pub use shellrs::cli::{AgentKind, IntegrationPaths};
pub use shellrs::connection::{
    ConnectionPrompt, ConnectionPromptField, ConnectionPromptKind, ConnectionPromptReply,
    ConnectionTester, Latency, LoginTest, TelnetTest, TrustCallback,
};
pub use shellrs::explorer::ExplorerId;
pub use shellrs::forward::{
    ForwardCommand, ForwardEvent, ForwardStatus, ForwardTransport, ForwardTransportProvider,
};
pub use shellrs::host::{
    AuthKind, ConnectionState, CredentialDraft, CredentialId, CredentialKind, ForwardDraft,
    ForwardEndpoint, ForwardId, ForwardKind, ForwardRule, GeneratedKey, GroupDraft, GroupId,
    HostDatabase, HostDraft, HostId, HostLogin, HostOs, HostStore, JumpLogin, KeyAlgorithm,
    LoginMethod, LoginRoute, PastedKey, ProxyKind, ProxySettings, Route, SerialLine,
    read_public_key,
};
pub use shellrs::secrets::{InMemorySecretStore, SecretRef, SecretStore as _};
pub use shellrs::settings::{Appearance, InterfaceLanguage, SettingsStore};
pub use shellrs::sftp::{
    DirectoryEntry, DirectoryListing, EDIT_LIMIT, EntryKind, FileBytes, FileMetadata, FileStamp,
    LocalDirectoryProvider, ReadFailure, RemotePath, SaveFailure, SftpCommand, SftpEvent,
    SftpRoute, SftpTransport, SftpTransportProvider, TextFile, UploadRequest,
};
pub use shellrs::terminal::{
    FixedLocalTerminalTransportProvider, FixedRemoteTerminalTransportProvider,
    FixedSerialTerminalTransportProvider, FixedTelnetTerminalTransportProvider, LocalShell,
    LocalTerminalId, RemoteTerminalId, RemoteTerminalTransportProvider, SftpChannel,
    SharedTerminalTransportFactory, TerminalFont, TerminalLifecycle, TerminalSize,
    TerminalTransport, TerminalTransportCommand, TerminalTransportEvent, TerminalTransportFactory,
};
pub use shellrs::update::{
    Channel, InstallKind, Installer, Relaunch, Release, Staged, TrustedKeys, Unsupported,
    UpdateError, UpdateFeed, UpdateServices,
};
pub use shellrs::workspace::Workspace;
pub use std::sync::atomic::{AtomicUsize, Ordering};
pub use std::sync::{Arc, Mutex, mpsc};
pub use std::time::Duration;

mod connection;
mod credential;
mod forward;
mod sftp;
mod terminal;
mod tools;

pub use connection::*;
pub use credential::*;
pub use forward::*;
pub use sftp::*;
pub use terminal::*;
pub use tools::*;

/// Seeded host ids, in insertion order (see `HostStore::seed`).
pub const WEB_01: u64 = 1;
pub const WEB_02: u64 = 2;
pub const DB_01: u64 = 3;
/// The first SFTP tab a test opens; tab ids count up from 1.
pub const SFTP_TAB: u64 = 1;
pub const STAGING_API: u64 = 4;
pub const DEV_BOX: u64 = 6;
pub const INITIAL_WEB_TERMINAL: u64 = 1;
pub const INITIAL_STAGING_TERMINAL: u64 = 2;
pub const FIRST_NEW_TERMINAL: u64 = 3;
/// Seeded group ids, in insertion order: 生产, 测试, 开发.
pub const PRODUCTION: u64 = 1;
pub const DEVELOPMENT: u64 = 3;

pub fn open_workspace(cx: &mut TestAppContext) -> (WindowHandle<Root>, Entity<Workspace>) {
    open_workspace_with_store(cx, HostStore::seed())
}

/// Connection of the tab in front, independent of any presentation.
pub fn active_connection_state(workspace: &Entity<Workspace>, cx: &App) -> Option<ConnectionState> {
    let workspace = workspace.read(cx);
    match workspace.active_tab()? {
        CenterTab::Terminal(id) => {
            Some(workspace.remote_terminal(id)?.read(cx).connection_state(cx))
        }
        CenterTab::Explorer(id) => Some(workspace.explorer(id)?.read(cx).connection_state()),
        CenterTab::Editor(id) => {
            let explorer = workspace.editor(id)?.read(cx).explorer_id()?;
            Some(workspace.explorer(explorer)?.read(cx).connection_state())
        }
        _ => None,
    }
}

pub fn active_local_lifecycle(
    workspace: &Entity<Workspace>,
    cx: &App,
) -> Option<TerminalLifecycle> {
    let workspace = workspace.read(cx);
    let CenterTab::LocalTerminal(id) = workspace.active_tab()? else {
        return None;
    };
    Some(
        workspace
            .local_terminal(id)?
            .read(cx)
            .terminal()
            .read(cx)
            .lifecycle(cx),
    )
}

pub fn open_host_manager(
    cx: &mut TestAppContext,
    main: WindowHandle<Root>,
    workspace: &Entity<Workspace>,
) -> WindowHandle<Root> {
    in_frame(cx, main, |window, cx| {
        window.dispatch_action(Box::new(shellrs::app::ShowHosts), cx)
    });
    cx.update(|cx| {
        workspace
            .read(cx)
            .host_manager_window()
            .expect("host management window opened")
            .downcast::<Root>()
            .unwrap()
    })
}

pub fn management_window(
    cx: &mut TestAppContext,
    handle: WindowHandle<Root>,
) -> WindowHandle<Root> {
    handle
        .read_with(cx, |root, cx| {
            root.view()
                .clone()
                .downcast::<Workspace>()
                .ok()
                .and_then(|workspace| workspace.read(cx).host_manager_window())
                .map(|window| window.downcast::<Root>().unwrap())
                .unwrap_or(handle)
        })
        .unwrap()
}

/// Production loads the store from the database; the tests hand one in
/// directly so they get the fixed shape `HostStore::seed` describes.
pub fn open_workspace_with_store(
    cx: &mut TestAppContext,
    store: HostStore,
) -> (WindowHandle<Root>, Entity<Workspace>) {
    open_workspace_with_tester(cx, store, Arc::new(FakeConnectionTester::default()))
}

pub fn one_host_store(auth: AuthKind) -> (HostStore, HostId) {
    let mut store = HostStore::empty();
    let id = store.insert_unnotified(HostDraft::new(
        "prompt-host",
        "example.test",
        22,
        "tester",
        auth,
        None,
    ));
    (store, id)
}

/// What every fixture starts with: ShellRS's globals, in Chinese, the
/// language the assertions are written in. The test's interface language is
/// its own, so one that switches to English switches no other test.
pub fn init_app(cx: &mut TestAppContext) {
    shellrs::i18n::isolate_thread();
    shellrs::i18n::set_locale("zh-CN");
    cx.update(shellrs::init);
}

/// Settings kept in memory, in Chinese: by default the interface follows the
/// system, and CI's is English.
pub fn settings_store() -> SettingsStore {
    let mut store = SettingsStore::in_memory();
    store.update_unnotified(|settings| settings.language = InterfaceLanguage::SimplifiedChinese);
    store
}

pub fn appearance_dropdown(window: &mut gpui_kit::Window, item: usize) -> Option<String> {
    window
        .within("settings")
        .within("group-0")
        .within(format!("item-{item}"))
        .find("btn")
        .label()
        .map(str::to_string)
}

/// `TestWindowExt::click` sends no modifiers, and file lists need ⌘ and
/// Shift clicks.
pub fn modified_click(
    window: &mut gpui_kit::Window,
    pane: (&'static str, u64),
    row: &str,
    modifiers: gpui_kit::Modifiers,
    cx: &mut App,
) {
    let row = ElementId::Name(row.to_string().into());
    let position = window.within(pane).find(row).bounds().center();
    click_at(window, position, modifiers, cx);
}

/// A left click at a point, for places without an element of their own.
pub fn click_at(
    window: &mut gpui_kit::Window,
    position: gpui_kit::Point<gpui_kit::Pixels>,
    modifiers: gpui_kit::Modifiers,
    cx: &mut App,
) {
    use gpui_kit::{MouseDownEvent, MouseUpEvent};
    window.dispatch_event(
        MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers,
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
    window.dispatch_event(
        MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers,
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(
        MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers,
            click_count: 1,
        }
        .to_platform_input(),
        cx,
    );
}

/// Whether a button accepts input. gpui-base does not report `disabled` for
/// buttons, but a disabled one also leaves the focus order.
pub fn enabled(button: &gpui_kit::test::ElementSnapshot) -> bool {
    button.focused().is_some()
}

/// What a remote tab says of its own connection. Connected is the normal
/// state; the title's tooltip and accessibility label report other states.
pub fn tab_connection_state(
    window: &mut gpui_kit::Window,
    tab: impl Into<ElementId>,
) -> Option<String> {
    let label = window.within(tab).find("title").label()?.to_string();
    (label != ConnectionState::Connected.label().as_ref()).then_some(label)
}

/// Draw a frame, run `body` against it, and let what it started settle.
pub fn in_frame<R>(
    cx: &mut TestAppContext,
    handle: WindowHandle<Root>,
    body: impl FnOnce(&mut gpui_kit::Window, &mut App) -> R,
) -> R {
    let result = cx
        .update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            body(window, cx)
        })
        .unwrap();
    cx.run_until_parked();
    result
}

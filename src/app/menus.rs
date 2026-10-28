//! macOS menu bar. Feature commands use the same actions as the workspace;
//! only application and native window operations are handled globally.

use gpui_kit::component::input;
use gpui_kit::{App, Global, Menu, MenuItem, OsAction, SystemMenuType, Window};

use super::{
    About, ClearTerminal, CloseActiveTab, EditorCommand, EditorShortcut, FindInTerminal,
    FindNextInTerminal, FindPreviousInTerminal, HideApp, HideOtherApps, ImportKnownHosts,
    ImportSshConfig, MainWindow, MinimizeWindow, NewGroup, NewHost, NewLocalTerminal,
    NewTemporaryConnection, NextTab, OpenChangelog, OpenSettings, OpenWebsite, PreviousTab,
    QuickConnect, Quit, ShowAllApps, ShowCredentials, ShowForwards, ShowHosts, ShowMainWindow,
    ToggleFullScreen, ToggleHostPanel, ToggleTheme, ToggleTool, ToggleToolSidebar, ToolKind,
    ZoomIn, ZoomOut, ZoomReset, ZoomTerminalIn, ZoomTerminalOut, ZoomTerminalReset, ZoomWindow,
};
use crate::i18n::t;

pub(super) fn init(cx: &mut App) {
    cx.on_action(|_: &About, _| show_about());
    cx.on_action(|_: &HideApp, cx| cx.hide());
    cx.on_action(|_: &HideOtherApps, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAllApps, cx| cx.unhide_other_apps());
    cx.on_action(|_: &MinimizeWindow, cx| {
        with_front_window(cx, |window, _| window.minimize_window());
    });
    cx.on_action(|_: &ZoomWindow, cx| {
        with_front_window(cx, |window, _| window.zoom_window());
    });
    cx.on_action(|_: &ToggleFullScreen, cx| {
        with_front_window(cx, |window, _| window.toggle_fullscreen());
    });
    cx.on_action(|_: &ShowMainWindow, cx| {
        if let Some(MainWindow(window)) = cx.try_global().copied() {
            cx.defer(move |cx| {
                window
                    .update(cx, |_, window, cx| super::bring_forward(window, cx))
                    .ok();
            });
        }
    });
}

fn with_front_window(cx: &mut App, command: impl FnOnce(&mut Window, &mut App) + 'static) {
    if let Some(window) = cx.active_window() {
        // Global actions bubble out of a window update. Re-enter after it ends.
        cx.defer(move |cx| {
            window.update(cx, |_, window, cx| command(window, cx)).ok();
        });
    }
}

pub(super) fn refresh(cx: &mut App) {
    cx.set_menus(application_menus(cx));
    register_system_menus(cx);
}

#[derive(Default, PartialEq, Eq)]
struct ToolMenu {
    offered: Vec<ToolKind>,
    showing: Option<ToolKind>,
}

impl Global for ToolMenu {}

pub(super) fn sync_tools(offered: Vec<ToolKind>, showing: Option<ToolKind>, cx: &mut App) {
    let state = ToolMenu { offered, showing };
    if cx.try_global::<ToolMenu>() == Some(&state) {
        return;
    }
    cx.set_global(state);
    refresh(cx);
}

/// The application's localized native menus, built from the current command state.
pub fn application_menus(cx: &App) -> Vec<Menu> {
    vec![
        Menu::new("Termirs").items([
            MenuItem::action(t!("app.menu.about"), About),
            MenuItem::separator(),
            MenuItem::action(t!("app.menu.settings"), OpenSettings),
            MenuItem::separator(),
            MenuItem::os_submenu(t!("app.menu.services"), SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action(t!("app.menu.hide"), HideApp),
            MenuItem::action(t!("app.menu.hide_others"), HideOtherApps),
            MenuItem::action(t!("app.menu.show_all"), ShowAllApps),
            MenuItem::separator(),
            // The global Quit handler runs the existing unsaved-editor guard.
            MenuItem::action(t!("app.menu.quit"), Quit),
        ]),
        Menu::new(t!("app.menu.file")).items([
            MenuItem::action(t!("app.menu.new_host"), NewHost),
            MenuItem::action(t!("app.menu.new_group"), NewGroup),
            MenuItem::action(t!("app.menu.new_local_terminal"), NewLocalTerminal),
            MenuItem::separator(),
            MenuItem::action(t!("app.menu.temporary_connection"), NewTemporaryConnection),
            MenuItem::action(t!("app.menu.quick_connect"), QuickConnect),
            MenuItem::submenu(Menu::new(t!("app.menu.import")).items([
                MenuItem::action(t!("app.menu.import_ssh_config"), ImportSshConfig),
                MenuItem::action(t!("app.menu.import_known_hosts"), ImportKnownHosts),
            ])),
            MenuItem::separator(),
            MenuItem::action(t!("app.menu.save"), EditorShortcut(EditorCommand::Save)),
            MenuItem::action(t!("app.menu.close_tab"), CloseActiveTab),
        ]),
        Menu::new(t!("app.menu.edit")).items([
            MenuItem::os_action(t!("app.menu.undo"), input::Undo, OsAction::Undo),
            MenuItem::os_action(t!("app.menu.redo"), input::Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action(t!("app.menu.cut"), input::Cut, OsAction::Cut),
            MenuItem::os_action(t!("app.menu.copy"), input::Copy, OsAction::Copy),
            MenuItem::os_action(t!("app.menu.paste"), input::Paste, OsAction::Paste),
            MenuItem::os_action(
                t!("app.menu.select_all"),
                input::SelectAll,
                OsAction::SelectAll,
            ),
        ]),
        Menu::new(t!("app.menu.view")).items([
            MenuItem::action(t!("app.menu.hosts"), ShowHosts),
            MenuItem::action(t!("app.menu.forwards"), ShowForwards),
            MenuItem::action(t!("app.menu.credentials"), ShowCredentials),
            MenuItem::separator(),
            MenuItem::action(t!("app.shortcut.toggle_sidebar"), ToggleHostPanel),
            MenuItem::separator(),
            MenuItem::action(t!("app.shortcut.zoom_in"), ZoomIn),
            MenuItem::action(t!("app.shortcut.zoom_out"), ZoomOut),
            MenuItem::action(t!("app.shortcut.zoom_reset"), ZoomReset),
            MenuItem::separator(),
            MenuItem::action(t!("app.menu.toggle_theme"), ToggleTheme),
        ]),
        Menu::new(t!("app.menu.terminal")).items([
            MenuItem::action(t!("app.menu.find"), FindInTerminal),
            MenuItem::action(t!("app.shortcut.terminal_find_next"), FindNextInTerminal),
            MenuItem::action(
                t!("app.shortcut.terminal_find_previous"),
                FindPreviousInTerminal,
            ),
            MenuItem::separator(),
            MenuItem::action(t!("app.shortcut.terminal_clear"), ClearTerminal),
            MenuItem::separator(),
            MenuItem::action(t!("app.shortcut.terminal_zoom_in"), ZoomTerminalIn),
            MenuItem::action(t!("app.shortcut.terminal_zoom_out"), ZoomTerminalOut),
            MenuItem::action(t!("app.shortcut.terminal_zoom_reset"), ZoomTerminalReset),
        ]),
        Menu::new(t!("app.menu.window")).items([
            MenuItem::action(t!("app.menu.minimize"), MinimizeWindow),
            MenuItem::action(t!("app.menu.zoom_window"), ZoomWindow),
            MenuItem::action(t!("app.menu.full_screen"), ToggleFullScreen),
            MenuItem::separator(),
            MenuItem::action(t!("app.shortcut.next_tab"), NextTab),
            MenuItem::action(t!("app.shortcut.previous_tab"), PreviousTab),
            MenuItem::separator(),
            MenuItem::action(t!("app.menu.main_window"), ShowMainWindow),
        ]),
        tool_menu(cx),
        Menu::new(t!("app.menu.help")).items([
            MenuItem::action(t!("app.menu.website"), OpenWebsite),
            MenuItem::action(t!("app.menu.changelog"), OpenChangelog),
        ]),
    ]
}

fn tool_menu(cx: &App) -> Menu {
    let empty = ToolMenu::default();
    let state = cx.try_global::<ToolMenu>().unwrap_or(&empty);
    let mut items: Vec<_> = state
        .offered
        .iter()
        .map(|&tool| {
            MenuItem::action(tool.label(), ToggleTool(tool)).checked(state.showing == Some(tool))
        })
        .collect();
    if !items.is_empty() {
        items.push(MenuItem::separator());
        items.push(MenuItem::action(
            t!("app.shortcut.toggle_tool_sidebar"),
            ToggleToolSidebar,
        ));
    }
    Menu::new(t!("app.menu.tools"))
        .disabled(state.offered.is_empty())
        .items(items)
}

fn register_system_menus(cx: &App) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::NSString;

    // The headless platform has no native menu bar.
    if cx.get_menus().is_none() {
        return;
    }
    let Some(main_thread) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(main_thread);
    let Some(menu) = app.mainMenu() else { return };
    let submenu = |title: &str| {
        menu.itemWithTitle(&NSString::from_str(title))
            .and_then(|item| item.submenu())
    };
    // GPUI only recognizes the English "Window" title automatically.
    // Register the localized menus too, including the actual Services submenu.
    app.setWindowsMenu(submenu(&t!("app.menu.window")).as_deref());
    app.setHelpMenu(submenu(&t!("app.menu.help")).as_deref());
    if let Some(application) = submenu("Termirs") {
        let services = application
            .itemWithTitle(&NSString::from_str(&t!("app.menu.services")))
            .and_then(|item| item.submenu());
        app.setServicesMenu(services.as_deref());
    }
}

fn show_about() {
    use objc2::{MainThreadMarker, runtime::AnyObject};
    use objc2_app_kit::{
        NSAboutPanelOptionApplicationName, NSAboutPanelOptionApplicationVersion,
        NSAboutPanelOptionVersion, NSApplication,
    };
    use objc2_foundation::{NSDictionary, NSString};

    let Some(main_thread) = MainThreadMarker::new() else {
        return;
    };
    let name = NSString::from_str("Termirs");
    let version = NSString::from_str(env!("CARGO_PKG_VERSION"));
    let empty = NSString::from_str("");
    // SAFETY: the three About option keys require NSString values. Explicit
    // values also give `cargo run` the product name and version without a bundle.
    unsafe {
        let options = NSDictionary::from_slices(
            &[
                NSAboutPanelOptionApplicationName,
                NSAboutPanelOptionApplicationVersion,
                NSAboutPanelOptionVersion,
            ],
            &[
                &*name as &AnyObject,
                &*version as &AnyObject,
                &*empty as &AnyObject,
            ],
        );
        NSApplication::sharedApplication(main_thread)
            .orderFrontStandardAboutPanelWithOptions(&options);
    }
}

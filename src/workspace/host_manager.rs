//! Hosts, forwarding rules and credentials share one management window.
//! Workspace owns commands and running connections; forms belong to this window.

use gpui_kit::component::{
    ActiveTheme as _, IconName, Root, Sizable as _, Theme, TitleBar, WindowExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::DropdownMenu as _,
    notification::Notification,
    tab::{Tab, TabBar},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::*;
use crate::credential::CredentialPanel;
use crate::forward::ForwardPanel;
use crate::host::{HostPanel, HostStoreEvent, import_menu_items};
use crate::i18n::{UiLocale, t};

use super::Workspace;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ManagerSection {
    Hosts,
    Forwards,
    Credentials,
}

impl ManagerSection {
    fn index(self) -> usize {
        match self {
            Self::Hosts => 0,
            Self::Forwards => 1,
            Self::Credentials => 2,
        }
    }
}

pub(super) struct HostManagerWindow {
    handle: WindowHandle<Root>,
    view: WeakEntity<HostManager>,
}

struct HostManager {
    workspace: WeakEntity<Workspace>,
    main: AnyWindowHandle,
    target: FocusHandle,
    panel: Entity<HostPanel>,
    forwards: Entity<ForwardPanel>,
    credentials: Entity<CredentialPanel>,
    section: ManagerSection,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl HostManager {
    fn show(&mut self, section: ManagerSection, window: &mut Window, cx: &mut Context<Self>) {
        self.section = section;
        self.focus_search(window, cx);
        cx.notify();
    }

    fn focus_search(&self, window: &mut Window, cx: &mut Context<Self>) {
        match self.section {
            ManagerSection::Hosts => self
                .panel
                .update(cx, |panel, cx| panel.focus_search(window, cx)),
            ManagerSection::Forwards => self
                .forwards
                .update(cx, |panel, cx| panel.focus_search(window, cx)),
            ManagerSection::Credentials => self
                .credentials
                .update(cx, |panel, cx| panel.focus_search(window, cx)),
        }
    }

    /// Call the existing workspace handler with this window, so forms and
    /// confirmations belong to the host list that requested them.
    fn local<A: Action>(
        &self,
        handler: fn(&mut Workspace, &A, &mut Window, &mut Context<Workspace>),
    ) -> impl Fn(&A, &mut Window, &mut App) + 'static {
        let workspace = self.workspace.clone();
        move |action, window, cx| {
            let _ = workspace.update(cx, |workspace, cx| handler(workspace, action, window, cx));
        }
    }

    /// Defer across windows, then dispatch on the workspace's action path.
    /// Neither the active terminal nor its connection can belong to this window.
    fn main<A: Action>(&self) -> impl Fn(&A, &mut Window, &mut App) + 'static {
        let main = self.main;
        let target = self.target.clone();
        move |action, _, cx| {
            let action = action.boxed_clone();
            let target = target.clone();
            cx.defer(move |cx| {
                let _ = main.update(cx, |_, window, cx| {
                    window.activate_window();
                    target.dispatch_action(action.as_ref(), window, cx);
                });
            });
        }
    }
}

impl Render for HostManager {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (new_host, new_group, sections) = (
            self.focus_handle.clone(),
            self.focus_handle.clone(),
            self.focus_handle.clone(),
        );
        let panel: AnyView = match self.section {
            ManagerSection::Hosts => self.panel.clone().into(),
            ManagerSection::Forwards => self.forwards.clone().into(),
            ManagerSection::Credentials => self.credentials.clone().into(),
        };
        let buttons = match self.section {
            ManagerSection::Hosts => Vec::new(),
            ManagerSection::Forwards => self
                .forwards
                .read(cx)
                .toolbar_buttons()
                .into_iter()
                .map(|button| button.small().label(t!("forward.menu.new")))
                .collect(),
            ManagerSection::Credentials => self
                .credentials
                .read(cx)
                .toolbar_buttons()
                .into_iter()
                .map(|button| button.small().label(t!("credential.menu.new")))
                .collect(),
        };
        v_flex()
            .id("host-manager")
            .test_support()
            .key_context(HOST_MANAGER_CONTEXT)
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().sidebar_foreground)
            .on_action(self.local(Workspace::on_new_host))
            .on_action(self.local(Workspace::on_edit_host))
            .on_action(self.local(Workspace::on_duplicate_host))
            .on_action(self.local(Workspace::on_move_host_node))
            .on_action(self.local(Workspace::on_delete_host))
            .on_action(self.local(Workspace::on_new_host_in_group))
            .on_action(self.local(Workspace::on_new_group))
            .on_action(self.local(Workspace::on_new_child_group))
            .on_action(self.local(Workspace::on_rename_group))
            .on_action(self.local(Workspace::on_delete_group))
            .on_action(self.local(Workspace::on_expand_all_groups))
            .on_action(self.local(Workspace::on_collapse_all_groups))
            .on_action(self.local(Workspace::on_import_ssh_config))
            .on_action(self.local(Workspace::on_import_known_hosts))
            .on_action(self.local(Workspace::on_copy_host_address))
            .on_action(self.local(Workspace::on_copy_host_id))
            .on_action(self.main::<ConnectHost>())
            .on_action(self.main::<ConnectGroup>())
            .on_action(self.main::<ReconnectTerminal>())
            .on_action(self.main::<OpenExplorer>())
            .on_action(self.main::<OpenSettings>())
            .on_action(self.main::<CheckForUpdates>())
            .on_action(self.main::<OpenWebsite>())
            .on_action(self.main::<OpenChangelog>())
            .on_action(self.main::<NewLocalTerminal>())
            .on_action(self.main::<NewTemporaryConnection>())
            .on_action(self.main::<QuickConnect>())
            .on_action(self.local(Workspace::on_show_hosts))
            .on_action(self.local(Workspace::on_show_forwards))
            .on_action(self.local(Workspace::on_show_credentials))
            .on_action(self.local(Workspace::on_new_forward))
            .on_action(self.local(Workspace::on_edit_forward))
            .on_action(self.local(Workspace::on_delete_forward))
            .on_action(self.local(Workspace::on_start_forward))
            .on_action(self.local(Workspace::on_stop_forward))
            .on_action(self.local(Workspace::on_new_credential))
            .on_action(self.local(Workspace::on_generate_credential_key))
            .on_action(self.local(Workspace::on_edit_credential))
            .on_action(self.local(Workspace::on_delete_credential))
            .on_action(self.local(Workspace::on_copy_credential_public_key))
            .on_action(self.main::<ToggleTheme>())
            .on_action(self.main::<ZoomIn>())
            .on_action(self.main::<ZoomOut>())
            .on_action(self.main::<ZoomReset>())
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.focus_search(window, cx);
            }))
            .on_action(|_: &CloseHostManager, window, _| window.remove_window())
            .on_action(|_: &CloseActiveTab, window, _| window.remove_window())
            .child(
                TitleBar::new().child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child(t!("host.manager.title")),
                ),
            )
            .child(
                TabBar::new("manager-sections")
                    .underline()
                    .small()
                    .selected_index(self.section.index())
                    .child(Tab::new().label(t!("host.panel.title")))
                    .child(Tab::new().label(t!("forward.panel.title")))
                    .child(Tab::new().label(t!("credential.panel.title")))
                    .on_click(move |ix, window, cx| {
                        let action: Box<dyn Action> = match ix {
                            0 => Box::new(ShowHosts),
                            1 => Box::new(ShowForwards),
                            _ => Box::new(ShowCredentials),
                        };
                        sections.dispatch_action(action.as_ref(), window, cx);
                    }),
            )
            .when(self.section == ManagerSection::Hosts, |body| {
                body.child(
                    h_flex()
                        .p_2()
                        .gap_2()
                        .border_b_1()
                        .border_color(cx.theme().sidebar_border)
                        .child(
                            Button::new("new-host-panel")
                                .small()
                                .icon(IconName::Plus)
                                .label(t!("host.menu.new_host"))
                                .tooltip_with_action(t!("host.menu.new_host"), &NewHost, None)
                                .on_click(move |_, window, cx| {
                                    new_host.dispatch_action(&NewHost, window, cx)
                                }),
                        )
                        .child(
                            Button::new("new-group")
                                .ghost()
                                .small()
                                .icon(CatalogIcon::FolderPlus)
                                .label(t!("host.menu.new_group"))
                                .on_click(move |_, window, cx| {
                                    new_group.dispatch_action(&NewGroup, window, cx)
                                }),
                        )
                        .child(div().flex_1())
                        .child(
                            Button::new("host-manager-more")
                                .ghost()
                                .small()
                                .icon(IconName::Ellipsis)
                                .tooltip(t!("host.manager.more"))
                                .accessibility_label(t!("host.manager.more"))
                                .dropdown_menu(|menu, _, _| import_menu_items(menu)),
                        ),
                )
            })
            .when(self.section != ManagerSection::Hosts, |body| {
                body.child(
                    h_flex()
                        .p_2()
                        .gap_2()
                        .border_b_1()
                        .border_color(cx.theme().sidebar_border)
                        .children(buttons),
                )
            })
            .child(div().flex_1().min_h_0().child(panel))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_dialog_layer(window, cx))
            .children(
                Root::render_notification_layer(window, cx)
                    .map(|layer| deferred(layer).with_priority(gpui_kit::base::POPUP_PRIORITY - 1)),
            )
    }
}

impl Workspace {
    pub(super) fn on_show_hosts(
        &mut self,
        _: &ShowHosts,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_host_manager(ManagerSection::Hosts, window, cx);
    }

    pub(super) fn show_host_manager(
        &mut self,
        section: ManagerSection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(manager) = &self.host_manager
            && let Some(view) = manager.view.upgrade()
        {
            if manager.handle.window_id() == window.window_handle().window_id() {
                view.update(cx, |manager, cx| manager.show(section, window, cx));
                return;
            }
            if manager
                .handle
                .update(cx, |_, window, cx| {
                    window.activate_window();
                    view.update(cx, |manager, cx| manager.show(section, window, cx));
                })
                .is_ok()
            {
                return;
            }
        }
        let workspace = cx.weak_entity();
        let main = window.window_handle();
        let target = self.focus_handle.clone();
        let store = self.store.clone();
        let forwards = self.forwards.clone();
        // Window geometry is a platform API boundary, expressed in Pixels.
        let size = size(px(800.), px(720.));
        let display = window.display(cx);
        let bounds = if let Some(display) = &display {
            let visible = display.visible_bounds();
            let size = size.min(&visible.size);
            let mut bounds = Bounds::centered_at(window.bounds().center(), size);
            bounds.origin.x = bounds
                .left()
                .clamp(visible.left(), visible.right() - size.width);
            bounds.origin.y = bounds
                .top()
                .clamp(visible.top(), visible.bottom() - size.height);
            bounds
        } else {
            Bounds::centered_at(window.bounds().center(), size)
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            display_id: display.map(|display| display.id()),
            window_min_size: Some(gpui_kit::size(px(480.), px(360.))),
            app_id: Some(DESKTOP_APP_ID.into()),
            ..TitleBar::window_options()
        };
        let mut view = None;
        let opened = cx.open_window(options, |window, cx| {
            let manager = cx.new(|cx| {
                let focus_handle = cx.focus_handle();
                let panel = cx.new(|cx| HostPanel::new(store.clone(), window, cx));
                let forward_panel = cx.new(|cx| {
                    ForwardPanel::new(store.clone(), forwards, focus_handle.clone(), window, cx)
                });
                let credentials = cx.new(|cx| {
                    CredentialPanel::new(store.clone(), focus_handle.clone(), window, cx)
                });
                let manager_window = window.window_handle();
                let closing_workspace = workspace.clone();
                let subscriptions = vec![
                    cx.on_release(move |_, cx| {
                        let _ = closing_workspace.update(cx, |workspace, cx| {
                            workspace.cancel_prompts_in_window(manager_window, cx);
                        });
                    }),
                    cx.on_window_closed(move |cx, id| {
                        if id == main.window_id() {
                            let _ =
                                manager_window.update(cx, |_, window, _| window.remove_window());
                        }
                    }),
                    cx.observe_global_in::<Theme>(window, |_, window, cx| {
                        window.refresh();
                        cx.notify();
                    }),
                    cx.observe_global_in::<UiLocale>(window, |_, window, cx| {
                        window.set_window_title(&t!("host.manager.title"));
                        cx.notify();
                    }),
                    cx.subscribe_in(&store, window, |_, _, event, window, cx| {
                        if let HostStoreEvent::PersistFailed(message) = event {
                            window.push_notification(Notification::error(message.clone()), cx);
                        }
                    }),
                ];
                HostManager {
                    workspace,
                    main,
                    target,
                    panel,
                    forwards: forward_panel,
                    credentials,
                    section,
                    focus_handle,
                    _subscriptions: subscriptions,
                }
            });
            window.set_window_title(&t!("host.manager.title"));
            window.activate_window();
            manager.update(cx, |manager, cx| manager.focus_search(window, cx));
            view = Some(manager.downgrade());
            cx.new(|cx| Root::new(manager, window, cx))
        });
        match (opened, view) {
            (Ok(handle), Some(view)) => {
                self.host_manager = Some(HostManagerWindow { handle, view })
            }
            (Err(error), _) => window.push_notification(Notification::error(error.to_string()), cx),
            _ => {}
        }
    }

    /// The live management window, if it has been opened and not closed.
    pub fn host_manager_window(&self) -> Option<AnyWindowHandle> {
        self.host_manager
            .as_ref()
            .filter(|manager| manager.view.upgrade().is_some())
            .map(|manager| manager.handle.into())
    }

    pub(super) fn host_panel(&self, cx: &App) -> Option<Entity<HostPanel>> {
        let manager = self.host_manager.as_ref()?.view.upgrade()?;
        Some(manager.read(cx).panel.clone())
    }

    pub(super) fn close_host_manager(&mut self, cx: &mut App) {
        if let Some(manager) = self.host_manager.take() {
            let _ = manager
                .handle
                .update(cx, |_, window, _| window.remove_window());
        }
    }
}

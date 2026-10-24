use gpui_kit::component::{
    ActiveTheme as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    dock::{BasePanel, Panel, PanelControl, PanelEvent},
    h_flex, v_flex,
};
use gpui_kit::*;

use crate::app::{CatalogIcon, OpenSettings, ShowHosts};
use crate::i18n::t;

use super::tab_sidebar::TabSidebar;

/// The main window's open tabs and shortcuts to settings and host management.
pub struct Sidebar {
    tabs: Entity<TabSidebar>,
    target: FocusHandle,
}

impl Sidebar {
    pub fn new(tabs: Entity<TabSidebar>, target: FocusHandle) -> Self {
        Self { tabs, target }
    }

    pub fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.tabs.read(cx).focus_handle(cx), cx);
    }
}

impl EventEmitter<PanelEvent> for Sidebar {}

impl Focusable for Sidebar {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.tabs.read(cx).focus_handle(cx)
    }
}

impl BasePanel for Sidebar {
    fn panel_name(&self) -> &'static str {
        "Sidebar"
    }

    fn closable(&self, _: &App) -> bool {
        false
    }

    fn zoomable(&self, _: &App) -> bool {
        false
    }
}

impl Panel for Sidebar {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        t!("workspace.tabs.title")
    }

    fn zoom_control(&self, _: &App) -> Option<PanelControl> {
        None
    }

    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}

impl Render for Sidebar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = self.target.clone();
        let hosts = self.target.clone();
        v_flex()
            .id("workspace-sidebar")
            .test_support()
            .size_full()
            .text_color(cx.theme().sidebar_foreground)
            .child(div().flex_1().min_h_0().child(self.tabs.clone()))
            .child(
                h_flex()
                    .flex_shrink_0()
                    .p_2()
                    .gap_2()
                    .child(
                        Button::new("open-settings")
                            .ghost()
                            .small()
                            .icon(CatalogIcon::Settings)
                            .tooltip_with_action(
                                t!("workspace.sidebar.open_settings"),
                                &OpenSettings,
                                None,
                            )
                            .accessibility_label(t!("workspace.sidebar.open_settings"))
                            .on_click(move |_, window, cx| {
                                settings.dispatch_action(&OpenSettings, window, cx)
                            }),
                    )
                    .child(
                        Button::new("show-hosts")
                            .ghost()
                            .small()
                            .icon(CatalogIcon::Server)
                            .tooltip_with_action(t!("app.shortcut.show_hosts"), &ShowHosts, None)
                            .accessibility_label(t!("app.shortcut.show_hosts"))
                            .on_click(move |_, window, cx| {
                                hosts.dispatch_action(&ShowHosts, window, cx)
                            }),
                    ),
            )
    }
}

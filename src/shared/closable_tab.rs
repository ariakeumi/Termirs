use std::rc::Rc;

use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{ContextMenuExt as _, PopupMenu},
    tooltip::Tooltip,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::host::ConnectionState;
use crate::i18n::t;

type MenuBuilder = Rc<dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;

/// A dock tab title: an optional leading element, the label and, for closable panels, a
/// close button that dispatches the panel's close action. A panel may also
/// give the title a context menu, which opens on a right click anywhere on it.
///
/// Closing goes through an action rather than `TabGroup::close_panel` on
/// purpose: the group refuses to close the last panel of a region, while the
/// workspace wants every tab closable (an empty center shows the recent
/// hosts instead) and removes the panel from the dock itself.
///
/// The surrounding sidebar row owns selection; the title owns its context
/// menu and optional close command. Sidebar titles omit the close button.
#[derive(IntoElement)]
pub struct ClosableTabTitle {
    id: ElementId,
    leading: Option<AnyElement>,
    label: SharedString,
    close: Option<(ElementId, Box<dyn Action>)>,
    menu: Option<MenuBuilder>,
    modified: bool,
    connection_state: Option<ConnectionState>,
}

impl ClosableTabTitle {
    /// `id` identifies the tab and must be unique among the tabs of a bar:
    /// the context menu keeps its open state under it.
    pub fn new(
        id: impl Into<ElementId>,
        leading: impl IntoElement,
        label: impl Into<SharedString>,
    ) -> Self {
        Self::text(id, label).with_leading(leading)
    }

    /// A title without an icon or a reserved leading slot.
    pub fn text(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            leading: None,
            label: label.into(),
            close: None,
            menu: None,
            modified: false,
            connection_state: None,
        }
    }

    pub fn with_leading(mut self, leading: impl IntoElement) -> Self {
        self.leading = Some(leading.into_any_element());
        self
    }

    /// Show this tab's own connection state, never its host's aggregate state:
    /// a connection that is not up greys the label. The title's tooltip and
    /// accessibility label also report the state. Tabs that do not own a remote
    /// connection leave this unset.
    pub fn connection_state(mut self, state: ConnectionState) -> Self {
        self.connection_state = Some(state);
        self
    }

    /// Mark the tab as holding changes not saved yet: a dot after the label,
    /// as editors do.
    pub fn modified(mut self, modified: bool) -> Self {
        self.modified = modified;
        self
    }

    /// Show a close button that dispatches the panel's close action.
    /// Sidebar titles omit this and retain closing in their context menus.
    pub fn closable(mut self, id: impl Into<ElementId>, action: Box<dyn Action>) -> Self {
        self.close = Some((id.into(), action));
        self
    }

    /// Open a menu built by `builder` on a right click. The builder runs when
    /// the menu opens, not while the tab renders.
    pub fn context_menu(
        mut self,
        builder: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> Self {
        self.menu = Some(Rc::new(builder));
        self
    }
}

impl RenderOnce for ClosableTabTitle {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let offline = self
            .connection_state
            .is_some_and(|state| !state.is_connected());
        let row = h_flex()
            .id("title")
            .test_support()
            .w_full()
            .min_w_0()
            .gap_1()
            .children(self.leading)
            .when_some(self.connection_state, |row, state| {
                row.aria_label(state.label())
                    .tooltip(move |window, cx| Tooltip::new(state.label()).build(window, cx))
            })
            .child(
                div()
                    .id("label")
                    .test_support()
                    .aria_label(self.label.clone())
                    .flex_1()
                    .min_w_0()
                    // A long title, such as a command line a shell names the
                    // tab after, gives way rather than the tab bar.
                    .max_w(rems(20.))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .when(offline, |label| {
                        label.text_color(cx.theme().muted_foreground)
                    })
                    .child(self.label.clone()),
            )
            .when(self.modified, |row| {
                row.child(
                    div()
                        .id("modified")
                        .test_support()
                        .aria_label(t!("shared.tab.unsaved"))
                        .size_1p5()
                        .rounded_full()
                        .bg(cx.theme().muted_foreground),
                )
            })
            .when_some(self.close, |row, (id, action)| {
                row.child(
                    Button::new(id)
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .tooltip(t!("common.close"))
                        .on_click(move |_, window, cx| {
                            // Closing must not also select or drag the tab.
                            cx.stop_propagation();
                            window.dispatch_action(action.boxed_clone(), cx);
                        }),
                )
            });
        div()
            .id(self.id)
            .w_full()
            .min_w_0()
            .test_support()
            .aria_label(self.label)
            .child(match self.menu {
                Some(menu) => row
                    .context_menu(move |popup, window, cx| menu(popup, window, cx))
                    .into_any_element(),
                None => row.into_any_element(),
            })
    }
}

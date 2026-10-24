//! Open tabs by category, retaining dock order within each category.
//! Dock keeps content, activation and drag behavior;
//! this view owns the vertical navigation, scrolling and focus.

use std::time::Duration;

use gpui_kit::component::{
    ActiveTheme as _,
    dock::{DockPlacement, DragPanel, PaneRef, PanelHandle, PanelId, TabGroupContext},
    h_flex,
    kbd::Kbd,
    list::ListItem,
    scroll::ScrollableElement as _,
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{
    ActivateTab, CenterTab, OPEN_TABS_CONTEXT, SelectNextOpenTab, SelectPreviousOpenTab, ShowTabs,
    SwitchToTab,
};
use crate::editor::EditorPanel;
use crate::explorer::ExplorerPanel;
use crate::host::{GroupId, group_options};
use crate::i18n::t;
use crate::settings::SettingsPanel;
use crate::shared::RenamableTab as _;
use crate::terminal::{LocalTerminalPanel, TerminalPanel};

use super::Workspace;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TabCategory {
    Local,
    HostGroup(GroupId),
    Ungrouped,
    Sftp,
    Other,
}

impl TabCategory {
    fn heading_id(self) -> Option<ElementId> {
        match self {
            Self::Local => Some("tab-category-local".into()),
            Self::HostGroup(id) => Some(("tab-category-group", id.0).into()),
            Self::Ungrouped => Some("tab-category-ungrouped".into()),
            Self::Sftp => Some("tab-category-sftp".into()),
            Self::Other => None,
        }
    }
}

pub(super) struct TabRow {
    tab: CenterTab,
    category: TabCategory,
    id: PanelId,
    panel: PanelHandle,
    group: TabGroupContext,
    ix: usize,
}

impl TabRow {
    fn label(&self, cx: &App) -> SharedString {
        let view = self.panel.view();
        let label = match self.tab {
            CenterTab::Terminal(_) => view
                .downcast::<TerminalPanel>()
                .ok()
                .map(|panel| panel.read(cx).tab_title(cx)),
            CenterTab::LocalTerminal(_) => view
                .downcast::<LocalTerminalPanel>()
                .ok()
                .map(|panel| panel.read(cx).display_title(cx)),
            CenterTab::Explorer(_) => view
                .downcast::<ExplorerPanel>()
                .ok()
                .map(|panel| panel.read(cx).tab_title(cx)),
            CenterTab::Editor(_) => view
                .downcast::<EditorPanel>()
                .ok()
                .map(|panel| panel.read(cx).name().into()),
            CenterTab::Settings => Some(t!("settings.tab.title")),
        };
        label.unwrap_or_else(|| t!("workspace.tabs.title"))
    }
}

pub(super) struct TabSidebar {
    font_size: f32,
    rows: Vec<TabRow>,
    categories: Vec<(TabCategory, SharedString)>,
    active: Option<CenterTab>,
    target: FocusHandle,
    focus_handle: FocusHandle,
    scroll: ScrollHandle,
    pending_focus: Option<CenterTab>,
    subscriptions: Vec<Subscription>,
    shortcut_hints: bool,
    shortcut_hint_task: Option<Task<()>>,
    _activation: Subscription,
}

struct TabDragPreview {
    panel: PanelHandle,
    font_size: f32,
}

impl Render for TabDragPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(rems(15.))
            .h_10()
            .px_3()
            .py_2()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_size(px(self.font_size))
            .border_1()
            .border_color(cx.theme().border)
            .rounded(cx.theme().radius)
            .child(self.panel.title(window, cx))
    }
}

impl TabSidebar {
    pub(super) fn new(
        target: FocusHandle,
        font_size: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let activation = cx.observe_window_activation(window, |this, _, cx| {
            this.hide_shortcut_hints(cx);
        });
        Self {
            font_size,
            rows: Vec::new(),
            categories: Vec::new(),
            active: None,
            target,
            focus_handle: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            pending_focus: None,
            subscriptions: Vec::new(),
            shortcut_hints: false,
            shortcut_hint_task: None,
            _activation: activation,
        }
    }

    fn tab_keystroke(&self, digit: usize, window: &Window) -> Option<Keystroke> {
        let binding =
            window.highest_precedence_binding_for_action_in(&SwitchToTab(digit), &self.target)?;
        Some(binding.keystrokes().first()?.inner().clone())
    }

    pub(super) fn hide_shortcut_hints(&mut self, cx: &mut Context<Self>) {
        self.shortcut_hint_task = None;
        if self.shortcut_hints {
            self.shortcut_hints = false;
            cx.notify();
        }
    }

    pub(super) fn on_modifiers_changed(
        &mut self,
        event: &ModifiersChangedEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(stroke) = self.tab_keystroke(1, window).filter(|stroke| {
            stroke.modifiers != Modifiers::default() && stroke.modifiers == event.modifiers
        }) else {
            self.hide_shortcut_hints(cx);
            return;
        };
        if self.shortcut_hint_task.is_some() {
            return;
        }
        self.shortcut_hint_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            _ = this.update_in(cx, |this, window, cx| {
                if window.modifiers() == stroke.modifiers {
                    this.shortcut_hints = true;
                    cx.notify();
                }
            });
        }));
    }

    pub(super) fn set_font_size(&mut self, font_size: f32, cx: &mut Context<Self>) {
        if self.font_size != font_size {
            self.font_size = font_size;
            cx.notify();
        }
    }

    fn set_rows(
        &mut self,
        mut rows: Vec<TabRow>,
        categories: Vec<(TabCategory, SharedString)>,
        active: Option<CenterTab>,
        cx: &mut Context<Self>,
    ) {
        rows.sort_by_cached_key(|row| {
            categories
                .iter()
                .position(|(category, _)| *category == row.category)
                .unwrap_or(categories.len())
        });
        let sections_changed = self.categories != categories
            || self
                .rows
                .iter()
                .map(|row| row.category)
                .ne(rows.iter().map(|row| row.category));
        let reordered = self
            .rows
            .iter()
            .map(|row| row.id)
            .ne(rows.iter().map(|row| row.id));
        if reordered {
            // Titles and connection marks can change without a layout change.
            self.subscriptions = rows
                .iter()
                .filter_map(|row| {
                    let view = row.panel.view();
                    macro_rules! observe {
                        ($view:ty) => {
                            view.downcast::<$view>()
                                .ok()
                                .map(|entity| cx.observe(&entity, |_, _, cx| cx.notify()))
                        };
                    }
                    match row.tab {
                        CenterTab::Terminal(_) => observe!(TerminalPanel),
                        CenterTab::LocalTerminal(_) => observe!(LocalTerminalPanel),
                        CenterTab::Explorer(_) => observe!(ExplorerPanel),
                        CenterTab::Editor(_) => observe!(EditorPanel),
                        CenterTab::Settings => observe!(SettingsPanel),
                    }
                })
                .collect();
        }
        if (self.active != active || reordered || sections_changed)
            && let Some(ix) = rows.iter().position(|row| Some(row.tab) == active)
        {
            // Headings are immediate scroll children too. Count each section
            // before this row so activating a tab still reveals the tab itself.
            let mut previous = None;
            let headings = rows[..=ix]
                .iter()
                .filter(|row| {
                    let category = row.category;
                    let heading = previous != Some(category)
                        && (category != TabCategory::Other || previous.is_some());
                    previous = Some(category);
                    heading
                })
                .count();
            self.scroll.scroll_to_item(ix + headings);
        }
        self.rows = rows;
        self.categories = categories;
        self.active = active;
        cx.notify();
    }

    /// Keyboard shortcuts keep their dock-group scope, in the same category
    /// order as the sidebar. Splitting a group never numbers another group's tabs.
    pub(super) fn tabs_in_active_group(&self) -> Vec<CenterTab> {
        let Some(active) = self.rows.iter().find(|row| Some(row.tab) == self.active) else {
            return Vec::new();
        };
        self.rows
            .iter()
            .filter(|row| row.group.node() == active.group.node())
            .map(|row| row.tab)
            .collect()
    }

    /// Panels focus their content when Dock delivers activation on the next
    /// tick. Arrow navigation keeps the list focused after that delivery.
    pub(super) fn restore_navigation_focus(
        &mut self,
        tab: Option<CenterTab>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(pending) = self.pending_focus.take()
            && tab == Some(pending)
        {
            window.focus(&self.focus_handle, cx);
        }
    }
}

impl Focusable for TabSidebar {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TabSidebar {
    fn render_row(
        &self,
        row: &TabRow,
        hint: Option<Keystroke>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ListItem {
        let target = self.target.clone();
        let tab = row.tab;
        let selected = self.active == Some(tab);
        let drag = row
            .group
            .is_draggable()
            .then(|| row.group.drag_panel(row.ix, cx))
            .flatten();
        let drop_group = row.group.clone();
        let ix = row.ix;
        let panel = row.panel.clone();
        let label = row.label(cx);
        let title = row.panel.title(window, cx);
        let font_size = self.font_size;
        ListItem::new(("open-tab", row.id.as_u64()))
            .role(Role::Tab)
            .aria_label(label)
            .aria_selected(selected)
            // Keep the selected row's own surface, as the host list
            // does, without ListItem's outlined active highlight.
            .confirmed(selected)
            .h_10()
            .mx_2()
            .mb_1()
            .px_2()
            .min_w_0()
            // A user-selected font size is independent of the window's rem scale.
            .text_size(px(font_size))
            .rounded(cx.theme().radius_tokens().xl)
            .when(selected, |row| {
                row.bg(cx.theme().accent)
                    .shadow(cx.theme().shadow_tokens().sm)
            })
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap_2()
                    .child(div().flex_1().min_w_0().child(title))
                    .when_some(hint, |row, stroke| {
                        row.child(
                            div()
                                .id("tab-shortcut-hint")
                                .test_support()
                                .aria_label(Kbd::format(&stroke))
                                .flex_shrink_0()
                                .child(Kbd::new(stroke)),
                        )
                    }),
            )
            .on_click(move |_, window, cx| {
                target.dispatch_action(&ActivateTab::new(tab), window, cx)
            })
            .when_some(drag, |row, drag| {
                row.on_drag(drag, move |drag, offset, window, cx| {
                    cx.stop_propagation();
                    drag.set_drag_offset(offset);
                    drag.set_preview_size(size(window.rem_size() * 15., window.rem_size() * 2.5));
                    cx.new(|_| TabDragPreview {
                        panel: panel.clone(),
                        font_size,
                    })
                })
            })
            .when(row.group.is_droppable(), |row| {
                row.drag_over::<DragPanel>(|row, _, _, cx| {
                    row.border_t_2().border_color(cx.theme().drag_border)
                })
                .on_drop(move |drag: &DragPanel, window, cx| {
                    drop_group.drop_panel(drag.clone(), Some(ix), true, window, cx);
                })
            })
    }
}

impl Render for TabSidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let group_tabs = self.tabs_in_active_group();
        let mut rows = Vec::new();
        let mut previous = None;
        for row in &self.rows {
            let category = row.category;
            if previous != Some(category) {
                if let Some(id) = category.heading_id() {
                    let title = self
                        .categories
                        .iter()
                        .find(|(candidate, _)| *candidate == category)
                        .map(|(_, title)| title.clone())
                        .unwrap_or_default();
                    let count = self
                        .rows
                        .iter()
                        .filter(|row| row.category == category)
                        .count();
                    let tooltip = title.clone();
                    rows.push(
                        h_flex()
                            .id(id)
                            .test_support()
                            .role(Role::Heading)
                            .aria_label(title.clone())
                            .tooltip(move |window, cx| {
                                Tooltip::new(tooltip.clone()).build(window, cx)
                            })
                            .flex_shrink_0()
                            .min_w_0()
                            .mx_4()
                            .mb_2()
                            .when(previous.is_some(), |heading| heading.mt_4())
                            .gap_2()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(title),
                            )
                            .child(
                                div()
                                    .id("count")
                                    .test_support()
                                    .aria_label(count.to_string())
                                    .flex_shrink_0()
                                    .child(count.to_string()),
                            )
                            .into_any_element(),
                    );
                } else if previous.is_some() {
                    // Editors and settings remain reachable after the
                    // connection categories, without a misleading protocol label.
                    rows.push(div().h_4().flex_shrink_0().into_any_element());
                }
                previous = Some(category);
            }
            // 1–8 follow the visible order within this dock group; 9 always
            // selects its last tab, matching SwitchToTab.
            let hint = self
                .shortcut_hints
                .then_some(())
                .and_then(|_| group_tabs.iter().position(|tab| *tab == row.tab))
                .and_then(|ix| {
                    if ix < 8 {
                        Some(ix + 1)
                    } else if ix + 1 == group_tabs.len() {
                        Some(9)
                    } else {
                        None
                    }
                })
                .and_then(|digit| self.tab_keystroke(digit, window))
                .filter(|stroke| stroke.modifiers == window.modifiers());
            rows.push(self.render_row(row, hint, window, cx).into_any_element());
        }
        v_flex()
            .id("open-tabs")
            .test_support()
            .role(Role::TabList)
            .key_context(OPEN_TABS_CONTEXT)
            .track_focus(&self.focus_handle)
            .relative()
            .size_full()
            .min_w_0()
            .child(
                v_flex()
                    .id("open-tabs-scroll")
                    .relative()
                    .size_full()
                    .min_h_0()
                    .track_scroll(&self.scroll)
                    .when(!self.rows.is_empty(), |list| list.py_2())
                    .children(rows)
                    .overflow_y_scroll(),
            )
            // Keep the overlay out of the tracked children: its full height
            // plus the list's padding would otherwise create false overflow.
            .vertical_scrollbar(&self.scroll)
    }
}

impl Workspace {
    pub(super) fn sync_open_tabs(&mut self, cx: &mut Context<Self>) {
        let area = self.dock_area.read(cx);
        let store = self.store.read(cx);
        // Use the same depth-first order and full paths as host management.
        // Only snapshots reach the sidebar, including when a host was deleted
        // while its terminal remains open.
        let mut categories = vec![(TabCategory::Local, t!("workspace.tabs.local"))];
        categories.extend(
            group_options(store.groups(), &[])
                .into_iter()
                .map(|(id, title)| (TabCategory::HostGroup(id), title)),
        );
        categories.extend([
            (TabCategory::Ungrouped, t!("workspace.tabs.ungrouped")),
            (TabCategory::Sftp, t!("workspace.tabs.sftp")),
        ]);
        let mut ids = Vec::new();
        let mut groups = Vec::new();
        if let Some(tree) = area.layout(DockPlacement::Center) {
            tree.root().walk(&mut |node| {
                if let PaneRef::Tabs { panels, .. } = node.kind() {
                    groups.push(node.id());
                    ids.extend_from_slice(panels);
                }
            });
        }
        let rows = ids
            .into_iter()
            .filter_map(|id| {
                let tab = self.center_tab_for_panel(id)?;
                let category = match tab {
                    CenterTab::LocalTerminal(_) => TabCategory::Local,
                    CenterTab::Terminal(id) => self
                        .terminals
                        .get(&id)
                        .and_then(|panel| store.host(panel.read(cx).host_id()))
                        .and_then(|host| host.group)
                        .filter(|id| store.group(*id).is_some())
                        .map(TabCategory::HostGroup)
                        .unwrap_or(TabCategory::Ungrouped),
                    CenterTab::Explorer(_) => TabCategory::Sftp,
                    CenterTab::Editor(_) | CenterTab::Settings => TabCategory::Other,
                };
                let base = area.panel(id)?;
                if !base.visible(cx) {
                    return None;
                }
                let panel = PanelHandle::of(base)?.clone();
                let (group, _) = self.center_tab_location(tab, cx)?;
                let group = group.read(cx).context(cx);
                let ix = group
                    .panels()
                    .iter()
                    .position(|panel| panel.panel_id(cx) == id)?;
                Some(TabRow {
                    tab,
                    category,
                    id,
                    panel,
                    group,
                    ix,
                })
            })
            .collect();
        self.skin.set_center_groups(groups, cx);
        self.tab_sidebar.update(cx, |list, cx| {
            list.set_rows(rows, categories, self.active_tab, cx)
        });
    }

    pub(super) fn on_show_tabs(
        &mut self,
        _: &ShowTabs,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.dock_area.read(cx).is_dock_open(DockPlacement::Left) {
            self.dock_area.update(cx, |area, cx| {
                area.toggle_dock(DockPlacement::Left, window, cx);
            });
        }
        self.sidebar
            .update(cx, |sidebar, cx| sidebar.focus_search(window, cx));
        cx.notify();
    }

    pub(super) fn on_activate_tab(
        &mut self,
        action: &ActivateTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tab_sidebar
            .update(cx, |list, _| list.pending_focus = None);
        let Some((group, panel)) = self.center_tab_location(action.tab(), cx) else {
            return;
        };
        let Some(ix) = group
            .read(cx)
            .panels()
            .iter()
            .position(|candidate| candidate.panel_id(cx) == panel)
        else {
            return;
        };
        if self.dock_area.read(cx).is_zoomed() {
            let node = group.read(cx).node();
            self.dock_area
                .update(cx, |area, cx| area.set_zoomed_in(node, window, cx));
        }
        group.update(cx, |group, cx| group.select_tab(ix, window, cx));
        self.set_active_tab(Some(action.tab()), window, cx);
        self.focus_center(window, cx);
        cx.notify();
    }

    pub(super) fn on_select_next_open_tab(
        &mut self,
        _: &SelectNextOpenTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.step_open_tab(1, window, cx);
    }

    pub(super) fn on_select_previous_open_tab(
        &mut self,
        _: &SelectPreviousOpenTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.step_open_tab(-1, window, cx);
    }

    fn step_open_tab(&mut self, step: isize, window: &mut Window, cx: &mut Context<Self>) {
        let list = self.tab_sidebar.read(cx);
        let count = list.rows.len();
        if count == 0 {
            return;
        }
        let ix = list
            .rows
            .iter()
            .position(|row| Some(row.tab) == self.active_tab)
            .unwrap_or(0);
        let next = (ix as isize + step).rem_euclid(count as isize) as usize;
        let tab = list.rows[next].tab;
        let changed = self.active_tab != Some(tab);
        self.on_activate_tab(&ActivateTab::new(tab), window, cx);
        if changed {
            self.tab_sidebar
                .update(cx, |list, _| list.pending_focus = Some(tab));
        }
        window.focus(&self.tab_sidebar.read(cx).focus_handle(cx), cx);
    }
}

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::dock::{
    BasePanelView, DockArea, DockAreaRenderer, DockContext, DockEvent, DockPlacement, DockSkin,
    DropIndicator, NodeId, PanelState, TabGroupContext, TabGroupRenderer, TilesRenderer,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// GPUI owns the drag lifecycle; Dock owns sizing and its constraints.
#[derive(Clone)]
struct SidebarResize;

impl Render for SidebarResize {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// The workspace's dock appearance: gpui-kit's `DockSkin`, plus a view that
/// fills the center while it holds no tab, and 命令广播's command bar above
/// the center's tab groups while terminals are checked.
///
/// The dock draws nothing for an empty center and `DockAreaRenderer` has no
/// empty-center hook, so the placeholder rides on `center_frame`. It is drawn
/// deferred, above the empty layout the dock paints there, whenever the
/// workspace last said the center is empty (`set_center_empty`); the
/// workspace keeps that in step with `DockEvent::LayoutChanged`.
pub(super) struct WorkspaceDockSkin {
    skin: Rc<DockSkin>,
    // Center groups have no header; their tabs live in the sidebar.
    // The tabs sidebar omits its header; other side docks use the standard skin.
    center_groups: Rc<RefCell<Vec<NodeId>>>,
    area: WeakEntity<DockArea>,
    placeholder: AnyView,
    center_empty: Cell<bool>,
    command_bar: AnyView,
    command_bar_shown: Cell<bool>,
    sidebar_material: Rc<super::sidebar_material::SidebarMaterial>,
    resizing_sidebar: Rc<Cell<bool>>,
}

impl WorkspaceDockSkin {
    /// Build a `DockArea` wearing this skin, together with the handle its
    /// settings are changed through. `placeholder` is what the center shows
    /// while it is empty, `command_bar` what it shows above its tab groups
    /// while 命令广播 has terminals checked.
    pub(super) fn dock_area(
        id: &'static str,
        version: Option<usize>,
        placeholder: AnyView,
        command_bar: AnyView,
        window: &mut Window,
        cx: &mut App,
    ) -> (Entity<DockArea>, Rc<Self>) {
        let mut skin = None;
        let area = cx.new(|cx| {
            let this = Rc::new(Self {
                skin: DockSkin::new(cx),
                center_groups: Rc::default(),
                area: cx.weak_entity(),
                placeholder,
                center_empty: Cell::new(false),
                command_bar,
                command_bar_shown: Cell::new(false),
                sidebar_material: Rc::new(super::sidebar_material::SidebarMaterial::new(window)),
                resizing_sidebar: Rc::default(),
            });
            skin = Some(this.clone());
            DockArea::new(id, version, window, cx).with_renderer(this)
        });
        // The closure above runs before `cx.new` returns.
        let skin = skin.expect("the skin is built inside the constructor");
        (area, skin)
    }

    pub(super) fn has_sidebar_material(&self) -> bool {
        self.sidebar_material.is_available()
    }

    pub(super) fn sync_sidebar_visibility(&self, area: &DockArea) {
        if area.is_zoomed() || !area.is_dock_open(DockPlacement::Left) {
            self.sidebar_material.hide();
        }
    }

    fn sidebar_background(&self, cx: &App) -> Hsla {
        if self.has_sidebar_material() {
            transparent_black()
        } else {
            cx.theme().sidebar
        }
    }

    pub(super) fn is_center_empty(&self) -> bool {
        self.center_empty.get()
    }

    pub(super) fn set_center_groups(&self, groups: Vec<NodeId>, cx: &mut App) {
        if *self.center_groups.borrow() != groups {
            *self.center_groups.borrow_mut() = groups;
            _ = self.area.update(cx, |_, cx| cx.notify());
        }
    }

    /// Show (`true`) or hide 命令广播's command bar.
    pub(super) fn set_command_bar_shown(&self, shown: bool, cx: &mut App) {
        if self.command_bar_shown.replace(shown) != shown {
            _ = self.area.update(cx, |_, cx| cx.notify());
        }
    }

    /// Show (`true`) or hide the placeholder. The skin is not an entity, so
    /// it redraws the area itself when the value changes.
    pub(super) fn set_center_empty(&self, empty: bool, cx: &mut App) {
        if self.center_empty.replace(empty) != empty {
            _ = self.area.update(cx, |_, cx| cx.notify());
        }
    }
}

impl DockAreaRenderer for WorkspaceDockSkin {
    fn frame(&self, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        self.skin.frame(window, cx)
    }

    fn split_frame(
        &self,
        node: NodeId,
        axis: Axis,
        window: &mut Window,
        cx: &mut App,
    ) -> Stateful<Div> {
        self.skin.split_frame(node, axis, window, cx)
    }

    fn center_frame(&self, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        self.skin
            .center_frame(window, cx)
            .bg(cx.theme().background)
            .relative()
            // The dock puts the center's tab groups after what the frame
            // holds, in a column: the bar sits above all of them.
            .when(
                self.command_bar_shown.get() && !self.center_empty.get(),
                |frame| frame.child(self.command_bar.clone()),
            )
            .when(self.center_empty.get(), |frame| {
                // Deferred, so it paints after the empty layout's own
                // background, which the dock draws later in the same frame.
                frame.child(deferred(
                    div().absolute().inset_0().child(self.placeholder.clone()),
                ))
            })
    }

    fn render_dock(
        &self,
        dock: &DockContext,
        content: AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        // macOS keeps window dragging and the traffic lights above the left
        // dock, so the center content can start at the window's top edge.
        let content = if cfg!(target_os = "macos") && dock.placement() == DockPlacement::Left {
            div()
                .flex()
                .flex_col()
                .size_full()
                .child(
                    super::title_bar::render_title_bar()
                        .bg(self.sidebar_background(cx))
                        .border_b_0(),
                )
                .child(div().flex_1().min_h_0().child(content))
                .into_any_element()
        } else {
            content
        };
        if dock.placement() == DockPlacement::Left {
            let material = self.sidebar_material.clone();
            let dark = cx.theme().is_dark();
            let resizing = self.resizing_sidebar.clone();
            let drag_state = resizing.clone();
            let dock = dock.clone();
            let area = self.area.clone();
            // Keep the framework's hit target and resize cursor, but draw no line.
            div()
                .flex()
                .size_full()
                .relative()
                .bg(self.sidebar_background(cx))
                .child(content)
                .child(
                    canvas(
                        move |bounds, _, _| material.sync(bounds, dark),
                        move |_, _, window, _| {
                            // Like DockSkin's tracker, listen at window level:
                            // the pointer leaves the handle as soon as it moves.
                            window.on_mouse_event({
                                let resizing = resizing.clone();
                                move |event: &MouseMoveEvent, phase, window, cx| {
                                    if phase.bubble() && resizing.get() {
                                        dock.resize_to(event.position, window, cx);
                                    }
                                }
                            });
                            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                                if phase.bubble() && resizing.replace(false) {
                                    _ = area.update(cx, |_, cx| cx.emit(DockEvent::LayoutChanged));
                                }
                            });
                        },
                    )
                    .absolute()
                    .inset_0(),
                )
                .child(
                    gpui_kit::base::resize_handle("resize-handle-left", Axis::Horizontal)
                        .placement(gpui_kit::base::Side::Left)
                        .with_appearance(Rc::new(|_, _, _| Some(Empty.into_any_element())))
                        .on_drag(SidebarResize, move |drag, _, _, cx| {
                            cx.stop_propagation();
                            drag_state.set(true);
                            cx.new(|_| drag.as_ref().clone())
                        }),
                )
                .into_any_element()
        } else {
            self.skin.render_dock(dock, content, window, cx)
        }
    }

    fn build_placeholder(
        &self,
        state: &PanelState,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Arc<dyn BasePanelView>> {
        self.skin.build_placeholder(state, window, cx)
    }

    fn tab_group_renderer(&self) -> Rc<dyn TabGroupRenderer> {
        Rc::new(WorkspaceTabBar {
            skin: self.skin.tab_group_renderer(),
            center_groups: self.center_groups.clone(),
            sidebar_material: self.has_sidebar_material(),
        })
    }

    fn tiles_renderer(&self) -> Rc<dyn TilesRenderer> {
        self.skin.tiles_renderer()
    }
}

/// Center groups render content without a header. Their vertical tab list
/// lives in the left sidebar; other dock groups retain the standard skin.
struct WorkspaceTabBar {
    skin: Rc<dyn TabGroupRenderer>,
    center_groups: Rc<RefCell<Vec<NodeId>>>,
    sidebar_material: bool,
}

impl TabGroupRenderer for WorkspaceTabBar {
    fn frame(&self, group: &TabGroupContext, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        let sidebar = group
            .active_panel()
            .is_some_and(|panel| panel.view().downcast::<super::sidebar::Sidebar>().is_ok());
        self.skin.frame(group, window, cx).when(sidebar, |frame| {
            frame.bg(if self.sidebar_material {
                transparent_black()
            } else {
                cx.theme().sidebar
            })
        })
    }
    fn content_frame(
        &self,
        group: &TabGroupContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Stateful<Div> {
        if self.center_groups.borrow().contains(&group.node()) {
            div().id("active-panel")
        } else {
            self.skin.content_frame(group, window, cx)
        }
    }
    fn render_tab_bar(
        &self,
        group: &TabGroupContext,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        if !self.center_groups.borrow().contains(&group.node()) {
            let tabs_sidebar = group
                .active_panel()
                .is_some_and(|panel| panel.view().downcast::<super::sidebar::Sidebar>().is_ok());
            if tabs_sidebar {
                return Empty.into_any_element();
            }
            return self.skin.render_tab_bar(group, window, cx);
        }
        Empty.into_any_element()
    }
    fn render_active_panel(
        &self,
        panel: AnyView,
        group: &TabGroupContext,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        self.skin.render_active_panel(panel, group, window, cx)
    }
    fn render_drop_indicator(
        &self,
        indicator: DropIndicator,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        self.skin.render_drop_indicator(indicator, window, cx)
    }
    fn render_empty(
        &self,
        group: &TabGroupContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        self.skin.render_empty(group, window, cx)
    }
}

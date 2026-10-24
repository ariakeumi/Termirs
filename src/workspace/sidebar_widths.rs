//! Remember the two sidebar widths without restoring the center's dock layout.

use gpui_kit::Context;
use gpui_kit::component::dock::DockPlacement;

use crate::settings::SidebarWidths;

use super::Workspace;

/// Leave room for the primary content and tool switch on a smaller window.
pub(super) fn fit_sidebar_widths(widths: SidebarWidths, window_width: f32) -> SidebarWidths {
    let minimum = SidebarWidths::MIN_LEFT + SidebarWidths::MIN_RIGHT;
    // Reserve a minimum panel extent for the content and tool switch;
    // wide user choices that still fit should survive a restart unchanged.
    let available = (window_width - SidebarWidths::MIN_LEFT).max(minimum);
    let extra = (widths.left() - SidebarWidths::MIN_LEFT) as f64
        + (widths.right() - SidebarWidths::MIN_RIGHT) as f64;
    if extra <= (available - minimum) as f64 {
        return widths;
    }
    let scale = (available - minimum) as f64 / extra;
    SidebarWidths::new(
        SidebarWidths::MIN_LEFT + ((widths.left() - SidebarWidths::MIN_LEFT) as f64 * scale) as f32,
        SidebarWidths::MIN_RIGHT
            + ((widths.right() - SidebarWidths::MIN_RIGHT) as f64 * scale) as f32,
    )
}

impl Workspace {
    /// `LayoutChanged` includes the end of a resize gesture. Hidden docks
    /// retain their size, so switching tools or collapsing a sidebar keeps it.
    pub(super) fn remember_sidebar_widths(&self, cx: &mut Context<Self>) {
        let area = self.dock_area.read(cx);
        let (Some(left), Some(right)) = (
            area.dock_size(DockPlacement::Left),
            area.dock_size(DockPlacement::Right),
        ) else {
            return;
        };
        let widths = SidebarWidths::new(left.into(), right.into());
        self.settings().update(cx, |store, cx| {
            store.update(|settings| settings.sidebar_widths = widths, cx);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::fit_sidebar_widths;
    use crate::settings::SidebarWidths;

    #[test]
    fn restored_sidebars_leave_room_for_the_content_on_a_smaller_window() {
        let widths = SidebarWidths::new(700., 600.);
        assert_eq!(fit_sidebar_widths(widths, 1600.), widths);
        let wide_left = SidebarWidths::new(800., 320.);
        assert_eq!(fit_sidebar_widths(wide_left, 1280.), wide_left);
        for width in [960., 1280.] {
            let fitted = fit_sidebar_widths(widths, width);
            assert!(fitted.left() >= SidebarWidths::MIN_LEFT);
            assert!(fitted.right() >= SidebarWidths::MIN_RIGHT);
            assert!((fitted.left() + fitted.right() - (width - 100.)).abs() < 0.001);
        }
        let minimum = SidebarWidths::new(100., 320.);
        assert_eq!(fit_sidebar_widths(minimum, 960.), minimum);
        let huge = fit_sidebar_widths(SidebarWidths::new(f32::MAX, f32::MAX), 960.);
        assert!((huge.left() + huge.right() - 860.).abs() < 0.001);
    }
}

//! A native sidebar behind GPUI's transparent Metal surface. The dock owns
//! the measured bounds; AppKit owns the material and accessibility behavior.

use gpui_kit::{Bounds, Pixels, Window};

pub(super) struct SidebarMaterial {
    #[cfg(target_os = "macos")]
    native: Option<macos::NativeSidebarMaterial>,
}

impl SidebarMaterial {
    pub(super) fn new(window: &Window) -> Self {
        #[cfg(target_os = "macos")]
        {
            Self {
                native: macos::NativeSidebarMaterial::new(window),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = window;
            Self {}
        }
    }

    pub(super) fn is_available(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.native.is_some()
        }
        #[cfg(not(target_os = "macos"))]
        false
    }

    pub(super) fn sync(&self, bounds: Bounds<Pixels>, dark: bool) {
        #[cfg(target_os = "macos")]
        if let Some(native) = &self.native {
            native.sync(bounds, dark);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (bounds, dark);
    }

    pub(super) fn hide(&self) {
        #[cfg(target_os = "macos")]
        if let Some(native) = &self.native {
            native.hide();
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::cell::Cell;

    use gpui_kit::{Bounds, Pixels, Window, WindowBackgroundAppearance};
    use objc2::{MainThreadMarker, MainThreadOnly as _, rc::Retained};
    use objc2_app_kit::{
        NSAppearance, NSAppearanceCustomization as _, NSAppearanceNameAqua,
        NSAppearanceNameDarkAqua, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
        NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub(super) struct NativeSidebarMaterial {
        effect: Retained<NSVisualEffectView>,
        content: Retained<NSView>,
        parent: Retained<NSView>,
        dark: Cell<Option<bool>>,
    }

    impl NativeSidebarMaterial {
        pub(super) fn new(window: &Window) -> Option<Self> {
            let main_thread = MainThreadMarker::new()?;
            let handle = HasWindowHandle::window_handle(window).ok()?;
            let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
                return None;
            };
            // SAFETY: GPUI supplies a live AppKit NSView for this window.
            // Retain it before storing it, and only access it on the UI thread.
            let content = unsafe { Retained::retain(handle.ns_view.cast::<NSView>().as_ptr()) }?;
            // SAFETY: content and its window are alive; retain the parent
            // for the lifetime of the material rather than keeping a weak pointer.
            let parent = unsafe { content.superview() }?;
            let effect = NSVisualEffectView::initWithFrame(
                NSVisualEffectView::alloc(main_thread),
                NSRect::ZERO,
            );
            // Sidebar's tint is almost opaque on recent macOS (84% in light
            // mode, 80% in dark mode). This chrome spans the traffic lights
            // and the tab list; use the more translucent HUD material so its
            // backdrop remains visible. AppKit still owns the blur, tint and
            // Reduce Transparency behavior.
            effect.setMaterial(NSVisualEffectMaterial::HUDWindow);
            effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
            // Keep the backdrop when the window loses focus. On macOS 27,
            // FollowsWindowActiveState removes the blur in inactive windows.
            effect.setState(NSVisualEffectState::Active);
            effect.setHidden(true);
            parent.addSubview_positioned_relativeTo(
                &effect,
                NSWindowOrderingMode::Below,
                Some(&content),
            );
            // Only transparent GPUI pixels expose the material. Terminals
            // and the rest of the workspace retain their own opaque fill.
            window.set_background_appearance(WindowBackgroundAppearance::Transparent);
            Some(Self {
                effect,
                content,
                parent,
                dark: Cell::new(None),
            })
        }

        pub(super) fn sync(&self, bounds: Bounds<Pixels>, dark: bool) {
            if bounds.size.width <= Pixels::ZERO || bounds.size.height <= Pixels::ZERO {
                self.hide();
                return;
            }
            // GPUI uses logical points with a top-left origin. AppKit may
            // use a bottom-left origin; conversion also handles its hierarchy.
            let rect = content_rect(bounds, self.content.bounds(), self.content.isFlipped());
            let frame = self.parent.convertRect_fromView(rect, Some(&self.content));
            if self.effect.frame() != frame {
                self.effect.setFrame(frame);
            }
            if self.dark.replace(Some(dark)) != Some(dark) {
                // SAFETY: AppKit exports these immutable appearance names.
                let name = unsafe {
                    if dark {
                        NSAppearanceNameDarkAqua
                    } else {
                        NSAppearanceNameAqua
                    }
                };
                let appearance = NSAppearance::appearanceNamed(name);
                self.effect.setAppearance(appearance.as_deref());
            }
            self.effect.setHidden(false);
        }

        pub(super) fn hide(&self) {
            self.effect.setHidden(true);
        }
    }

    impl Drop for NativeSidebarMaterial {
        fn drop(&mut self) {
            self.effect.removeFromSuperview();
        }
    }

    fn content_rect(bounds: Bounds<Pixels>, content: NSRect, flipped: bool) -> NSRect {
        let left = f64::from(bounds.origin.x.as_f32());
        let top = f64::from(bounds.origin.y.as_f32());
        let width = f64::from(bounds.size.width.as_f32());
        let height = f64::from(bounds.size.height.as_f32());
        let y = if flipped {
            content.origin.y + top
        } else {
            content.origin.y + content.size.height - top - height
        };
        NSRect::new(
            NSPoint::new(content.origin.x + left, y),
            NSSize::new(width, height),
        )
    }

    #[cfg(test)]
    mod tests {
        use super::content_rect;
        use gpui_kit::{Bounds, point, px, size};
        use objc2_foundation::{NSPoint, NSRect, NSSize};

        #[test]
        fn sidebar_frame_converts_top_left_points_without_retina_scaling() {
            let bounds = Bounds::new(point(px(0.), px(34.)), size(px(260.), px(566.)));
            let content = NSRect::new(NSPoint::new(0., 0.), NSSize::new(960., 600.));
            assert_eq!(
                content_rect(bounds, content, false),
                NSRect::new(NSPoint::new(0., 0.), NSSize::new(260., 566.))
            );
            assert_eq!(
                content_rect(bounds, content, true),
                NSRect::new(NSPoint::new(0., 34.), NSSize::new(260., 566.))
            );
        }

        #[test]
        fn sidebar_frame_preserves_offsets_and_fractional_widths() {
            let bounds = Bounds::new(point(px(8.), px(12.)), size(px(240.5), px(700.)));
            let content = NSRect::new(NSPoint::new(2., 4.), NSSize::new(1200., 800.));
            assert_eq!(
                content_rect(bounds, content, false),
                NSRect::new(NSPoint::new(10., 92.), NSSize::new(240.5, 700.))
            );
        }
    }
}

use gpui_kit::component::TitleBar;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{Styled as _, px};

/// Window chrome only; workspace commands live outside the title bar.
pub fn render_title_bar() -> TitleBar {
    TitleBar::new().when(cfg!(target_os = "macos"), |bar| {
        // Reserve room for the inset native controls, including their hit targets.
        // Window chrome is a platform boundary and does not scale with UI text.
        bar.h(px(56.))
    })
}

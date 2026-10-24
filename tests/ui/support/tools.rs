//! Tool selection follows the platform's command surface: the real native
//! menu definitions on macOS, and the button strip on other platforms.

use super::*;
use gpui_kit::Window;

#[cfg(target_os = "macos")]
fn menu_item(id: &str, cx: &App) -> Option<gpui_kit::MenuItem> {
    let tool = shellrs::app::ToolKind::from_key(id.strip_prefix("tool-")?)?;
    shellrs::app::application_menus(cx)
        .into_iter()
        .flat_map(|menu| menu.items)
        .find(|item| matches!(item, gpui_kit::MenuItem::Action { action, .. }
            if action.as_any().downcast_ref::<shellrs::app::ToggleTool>() == Some(&shellrs::app::ToggleTool(tool))))
}

pub fn toggle_tool(window: &mut Window, id: &str, cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        let item = menu_item(id, cx).expect("offered tool in the native menu");
        assert!(!item.is_disabled());
        let gpui_kit::MenuItem::Action { action, .. } = item else {
            unreachable!()
        };
        window.dispatch_action(action, cx);
    }
    #[cfg(not(target_os = "macos"))]
    window.click(id.to_string(), cx);
}

pub fn tool_offered(window: &mut Window, id: &str, cx: &App) -> bool {
    #[cfg(target_os = "macos")]
    {
        let _ = window;
        menu_item(id, cx).is_some_and(|item| !item.is_disabled())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = cx;
        window
            .try_find(id.to_string())
            .is_some_and(|item| item.visible())
    }
}

pub fn tool_checked(window: &mut Window, id: &str, cx: &App) -> Option<bool> {
    #[cfg(target_os = "macos")]
    {
        let _ = window;
        menu_item(id, cx).map(|item| item.is_checked())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = cx;
        window.find(id.to_string()).checked()
    }
}

pub fn has_tool_commands(window: &mut Window, cx: &App) -> bool {
    shellrs::app::ToolKind::ALL
        .into_iter()
        .any(|tool| tool_offered(window, &format!("tool-{}", tool.key()), cx))
}

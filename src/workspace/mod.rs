//! The window shell: title bar, dock area with its sidebars, the switch of
//! the right sidebar's tools and the start page (recent hosts)
//! the center shows while no tab is open.

mod analytics;
mod broadcast_bar;
mod cli_changes;
mod credentials;
mod dock_skin;
mod editors;
mod follow;
mod forwards;
mod front;
mod host_manager;
mod imports;
mod links;
mod notices;
mod projects;
mod recent_hosts;
mod sidebar;
mod sidebar_material;
mod sidebar_widths;
mod snippets;
mod tab_sidebar;
mod tabs;
mod terminal_groups;
mod title_bar;
mod tmux;
mod tool_sidebar;
mod tools;
mod updates;
mod window_state;
mod workspace_view;
mod zmodem;

pub use links::{open_link_once_open, open_target_once_open};
pub use window_state::WindowState;
pub use workspace_view::{Workspace, notify_once_open, window_options};

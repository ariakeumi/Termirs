//! Moving between the center's tabs from the keyboard: the next one, the
//! previous one, or one by its sidebar position, within the current tab's group.

use gpui_kit::*;

use crate::app::{ActivateTab, NextTab, PreviousTab, SwitchToTab};

use super::Workspace;

impl Workspace {
    pub(super) fn on_next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<Self>) {
        self.step_tab(1, window, cx);
    }

    pub(super) fn on_previous_tab(
        &mut self,
        _: &PreviousTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.step_tab(-1, window, cx);
    }

    /// From 1; 9 is the last, as in browsers. A position past the last
    /// does nothing.
    pub(super) fn on_switch_to_tab(
        &mut self,
        action: &SwitchToTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tabs = self.tab_sidebar.read(cx).tabs_in_active_group();
        let ix = if action.0 >= 9 {
            tabs.len().checked_sub(1)
        } else {
            action.0.checked_sub(1)
        };
        if let Some(tab) = ix.and_then(|ix| tabs.get(ix)) {
            self.on_activate_tab(&ActivateTab::new(*tab), window, cx);
        }
    }

    /// The tab `step` places away from the current one, around the ends.
    fn step_tab(&mut self, step: isize, window: &mut Window, cx: &mut Context<Self>) {
        let tabs = self.tab_sidebar.read(cx).tabs_in_active_group();
        let Some(ix) = tabs.iter().position(|tab| Some(*tab) == self.active_tab) else {
            return;
        };
        if tabs.len() > 1 {
            let next = (ix as isize + step).rem_euclid(tabs.len() as isize) as usize;
            self.on_activate_tab(&ActivateTab::new(tabs[next]), window, cx);
        }
    }
}

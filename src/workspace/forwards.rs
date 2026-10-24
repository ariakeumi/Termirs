//! The workspace's share of port forwarding: its management window, the
//! commands of the forward list, and what a running forward reports.

use std::rc::Rc;

use gpui_kit::component::{WindowExt as _, notification::Notification};
use gpui_kit::*;

use crate::app::{DeleteForward, EditForward, NewForward, ShowForwards, StartForward, StopForward};
use crate::forward::{ForwardManagerEvent, open_forward_dialog};
use crate::host::ForwardId;
use crate::i18n::t;
use crate::shared::confirm_delete;

use super::{
    host_manager::ManagerSection,
    workspace_view::{PromptOwner, Workspace},
};

impl Workspace {
    pub(super) fn on_show_forwards(
        &mut self,
        _: &ShowForwards,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_host_manager(ManagerSection::Forwards, window, cx);
    }

    pub(super) fn on_new_forward(
        &mut self,
        _: &NewForward,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        if store.ssh_hosts().next().is_none() {
            window.push_notification(
                Notification::warning(t!("workspace.forward.needs_host")),
                cx,
            );
            return;
        }
        open_forward_dialog(None, false, self.store.clone(), window, cx);
    }

    pub(super) fn on_edit_forward(
        &mut self,
        action: &EditForward,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = action.0;
        if self.store.read(cx).forward(id).is_none() {
            return;
        }
        let active = self.forwards.read(cx).is_active(id);
        open_forward_dialog(Some(id), active, self.store.clone(), window, cx);
    }

    pub(super) fn on_delete_forward(
        &mut self,
        action: &DeleteForward,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = action.0;
        let Some(title) = self.store.read(cx).forward(id).map(|rule| rule.title()) else {
            return;
        };
        let description = self
            .forwards
            .read(cx)
            .is_active(id)
            .then(|| t!("workspace.forward.delete_running"));
        let store = self.store.clone();
        confirm_delete(
            &title,
            description,
            // The manager stops a rule that leaves the store.
            Rc::new(move |_, cx| {
                store.update(cx, |store, cx| {
                    store.remove_forward(id, cx);
                });
            }),
            window,
            cx,
        );
    }

    pub(super) fn on_start_forward(
        &mut self,
        action: &StartForward,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.forwards
            .update(cx, |forwards, cx| forwards.start(action.0, cx));
        if let Some(rule) = self.store.read(cx).forward(action.0) {
            self.count_forward(rule.kind);
        }
    }

    pub(super) fn on_stop_forward(
        &mut self,
        action: &StopForward,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_forward_prompts(action.0, None, window, cx);
        self.forwards
            .update(cx, |forwards, cx| forwards.stop(action.0, cx));
    }

    pub(super) fn on_forward_event(
        &mut self,
        event: &ForwardManagerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ForwardManagerEvent::StatusChanged(id) => {
                // A question of a run that is over has nobody to answer to.
                let current = self.forwards.read(cx).generation(*id);
                self.cancel_forward_prompts(*id, current, window, cx);
                // The title bar counts the forwards that are running.
                cx.notify();
            }
            ForwardManagerEvent::PromptRequested(id, host, generation, prompt) => self
                .enqueue_prompt(
                    PromptOwner::Forward(*id, *generation),
                    *host,
                    prompt.clone(),
                    window,
                    cx,
                ),
            // The list may not be showing, and a forward that stopped on its
            // own is something the user was relying on.
            ForwardManagerEvent::Failed(id, reason) => {
                let title = self
                    .store
                    .read(cx)
                    .forward(*id)
                    .map(|rule| rule.title())
                    .unwrap_or_default();
                let notification = Notification::error(reason.clone())
                    .title(t!("workspace.forward.stopped", title = title));
                if let Some(target) = self
                    .host_manager_window()
                    .filter(|target| *target != window.window_handle())
                {
                    cx.defer(move |cx| {
                        let _ = target.update(cx, |_, window, cx| {
                            window.push_notification(notification, cx)
                        });
                    });
                } else {
                    window.push_notification(notification, cx);
                }
            }
        }
    }

    /// Drop the questions of a forward's runs other than `keep`: all of
    /// them when it is `None`.
    fn cancel_forward_prompts(
        &mut self,
        id: ForwardId,
        keep: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let stale = |owner: &PromptOwner| {
            matches!(owner, PromptOwner::Forward(forward, generation)
                if *forward == id && Some(*generation) != keep)
        };
        self.prompt_queue.retain(|(owner, _, _, _)| !stale(owner));
        if let Some((owner, _, _)) = self.active_prompt
            && stale(&owner)
        {
            self.cancel_prompts_for_owner(owner, window, cx);
        }
    }

    /// Who is asking, for a question that a forward raised: unlike a
    /// terminal or an SFTP tab it may have started on its own, with nothing
    /// on screen to say which connection the dialog is about.
    pub(super) fn forward_prompt_origin(&self, id: ForwardId, cx: &App) -> Option<String> {
        let store = self.store.read(cx);
        let rule = store.forward(id)?;
        let host = store.host(rule.host)?;
        Some(
            t!(
                "workspace.forward.prompt_origin",
                title = rule.title(),
                host = host.name
            )
            .to_string(),
        )
    }
}

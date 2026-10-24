//! The workspace's share of app launches: resolve saved host names/IDs,
//! or open links as external connections (外部连接):
//! a bastion host runs `ShellRS ssh://user@host:port`, the way it runs
//! Xshell, or `ShellRS sftp://…`, the way it runs WinSCP, and ShellRS
//! connects without saving the host. The host goes with its last tab.

use gpui_kit::component::{WindowExt as _, notification::Notification};
use gpui_kit::*;

use crate::cli::{OpenLink, OpenTarget};
use crate::host::{HostId, HostStore, LinkKind, SshLink};
use crate::i18n::t;

use super::workspace_view::Workspace;

impl Workspace {
    /// Open an external link or connect using a saved host's configuration.
    pub fn open_target(&mut self, target: OpenTarget, window: &mut Window, cx: &mut Context<Self>) {
        match target {
            OpenTarget::LocalDirectory { directory } => {
                self.open_local_directory(directory, window, cx);
            }
            OpenTarget::Link(link) => self.open_link(link, window, cx),
            OpenTarget::Host { host } => match saved_host_named(self.store.read(cx), &host) {
                Ok(id) => self.connect_host(id, window, cx),
                Err(problem) => window.push_notification(
                    Notification::error(problem).title(t!("workspace.launch.open_failed")),
                    cx,
                ),
            },
        }
    }

    /// Connect to what `link` names in a tab of its own across the window,
    /// a terminal or an SFTP tab by the link's scheme, or say what is wrong
    /// with it.
    pub fn open_link(&mut self, link: OpenLink, window: &mut Window, cx: &mut Context<Self>) {
        match SshLink::parse(&link.url, link.tab.as_deref()) {
            Ok(link) => {
                let kind = link.kind;
                let host = self
                    .store
                    .update(cx, |store, cx| store.insert_external(link, cx));
                match kind {
                    LinkKind::Ssh => self.connect_host(host, window, cx),
                    LinkKind::Sftp => self.open_explorer(host, window, cx),
                }
            }
            Err(problem) => window.push_notification(
                Notification::error(problem).title(t!("workspace.link.open_failed")),
                cx,
            ),
        }
    }

    /// Forget a temporary host once its last tab has closed, with whatever
    /// it was still asking.
    pub(super) fn forget_unused_temporary_host(
        &mut self,
        host: HostId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.store.read(cx).is_temporary(host) || self.has_tabs(host, cx) {
            return;
        }
        self.cancel_prompts_for_host(host, window, cx);
        self.store
            .update(cx, |store, cx| store.remove_temporary(host, cx));
    }
}

/// Open the link ShellRS was started with, once the window is there: an
/// error notification needs the window's `Root`, which is only in place
/// after the closure building the window returns (see
/// [`super::notify_once_open`]).
pub fn open_link_once_open(
    workspace: &Entity<Workspace>,
    link: OpenLink,
    window: &mut Window,
    cx: &mut App,
) {
    open_target_once_open(workspace, OpenTarget::Link(link), window, cx);
}

/// Defer a launch until `Root` and the restored tabs are ready.
pub fn open_target_once_open(
    workspace: &Entity<Workspace>,
    target: OpenTarget,
    window: &mut Window,
    cx: &mut App,
) {
    let workspace = workspace.downgrade();
    window.defer(cx, move |window, cx| {
        workspace
            .update(cx, |workspace, cx| {
                workspace.open_target(target, window, cx)
            })
            .ok();
    });
}

/// Names are exact and case-sensitive. An ID disambiguates duplicate names;
/// temporary connections are never candidates.
fn saved_host_named(store: &HostStore, name: &str) -> Result<HostId, SharedString> {
    if let Some(host) = store
        .hosts()
        .iter()
        .find(|host| host.public_id.as_str() == name)
    {
        return Ok(host.id);
    }
    let mut matches = store
        .hosts()
        .iter()
        .filter(|host| host.name.as_ref() == name);
    let Some(host) = matches.next() else {
        return Err(t!("workspace.launch.host_not_found", name = name));
    };
    if matches.next().is_some() {
        return Err(t!("workspace.launch.ambiguous_host", name = name));
    }
    Ok(host.id)
}

#[cfg(test)]
mod tests {
    use super::saved_host_named;
    use crate::host::{AuthKind, HostDraft, HostStore, SshLink};

    #[test]
    fn saved_host_launch_matches_exactly_and_ids_resolve_duplicate_names() {
        let mut store = HostStore::empty();
        let draft = || {
            HostDraft::new(
                "生产 Web",
                "web.test",
                2222,
                "deploy",
                AuthKind::Password,
                None,
            )
        };
        let first = store.insert_unnotified(draft());
        assert_eq!(saved_host_named(&store, "生产 Web").unwrap(), first);
        for name in ["生产 web", "生产", "web.test", " 生产 Web", "", "missing"] {
            assert!(saved_host_named(&store, name).is_err(), "{name}");
        }
        let second = store.insert_unnotified(draft());
        assert!(saved_host_named(&store, "生产 Web").is_err());
        for id in [first, second] {
            let public_id = store.host(id).unwrap().public_id.as_str();
            assert_eq!(saved_host_named(&store, public_id).unwrap(), id);
        }
    }

    #[test]
    fn saved_host_launch_ignores_temporary_hosts() {
        let mut store = HostStore::empty();
        let id = store.insert_external_unnotified(
            SshLink::parse("ssh://root@web.test", Some("Temporary")).unwrap(),
        );
        assert!(saved_host_named(&store, "Temporary").is_err());
        assert!(saved_host_named(&store, store.host(id).unwrap().public_id.as_str()).is_err());
    }
}

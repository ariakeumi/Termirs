use crate::support::*;
use shellrs::app::CloseHostManager;

#[gpui_kit::test]
fn host_manager_shortcut_opens_one_secondary_window(cx: &mut TestAppContext) {
    let (main, workspace) = open_workspace(cx);
    in_frame(cx, main, |window, cx| {
        assert!(window.try_find("host-tree").is_none());
        assert!(window.try_find("host-search").is_none());
        assert!(window.find("show-hosts").visible());
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-shift-h"
            } else {
                "ctrl-shift-h"
            },
            cx,
        );
    });
    let manager = cx
        .update(|cx| workspace.read(cx).host_manager_window())
        .unwrap();
    assert_ne!(manager, main.into());
    cx.update_window(manager, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("host-manager").visible());
        assert_eq!(window.find("host-search").focused(), Some(true));
        assert!(window.find(("host-row", WEB_01)).visible());
        window.input("staging", cx);
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-shift-h"
            } else {
                "ctrl-shift-h"
            },
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| workspace.read(cx).host_manager_window()),
        Some(manager)
    );
    cx.update_window(manager, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("host-search").value(), Some("staging"));
        assert!(window.try_find(("host-row", WEB_01)).is_none());
        assert!(window.find(("host-row", STAGING_API)).visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn host_manager_edits_hosts_and_connects_in_the_main_window(cx: &mut TestAppContext) {
    let (main, workspace) = open_workspace(cx);
    let manager = open_host_manager(cx, main, &workspace);
    in_frame(cx, manager, |window, cx| window.click("new-group", cx));
    in_frame(cx, manager, |window, cx| {
        window.click("group-name", cx);
        window.input("管理窗口分组", cx);
        window.click("commit", cx);
    });
    in_frame(cx, manager, |window, cx| window.click("new-host-panel", cx));
    in_frame(cx, manager, |window, cx| {
        window.click("host-name", cx);
        window.input("管理窗口主机", cx);
        window.click("host-address", cx);
        window.input("manager.test", cx);
        window.click("commit", cx);
    });
    let created = cx.update(|cx| {
        workspace
            .read(cx)
            .store()
            .read(cx)
            .hosts()
            .iter()
            .find(|host| host.name.as_ref() == "管理窗口主机")
            .unwrap()
            .id
    });
    in_frame(cx, manager, |window, cx| {
        assert!(window.find(("host-row", created.0)).visible());
        window.double_click(("host-row", DB_01), cx);
    });
    in_frame(cx, main, |window, _| {
        assert!(window.find(("terminal", FIRST_NEW_TERMINAL)).visible());
        assert!(window.try_find("host-tree").is_none());
    });
    in_frame(cx, manager, |window, cx| {
        window.click("host-search", cx);
        window.dispatch_action(Box::new(EditHost(created)), cx);
    });
    in_frame(cx, manager, |window, cx| {
        assert_eq!(window.find("host-name").value(), Some("管理窗口主机"));
        window.press("escape", cx);
    });
    assert!(
        cx.update(|cx| workspace.read(cx).host_manager_window())
            .is_some()
    );
    in_frame(cx, manager, |window, cx| {
        window.click("host-search", cx);
        window.dispatch_action(Box::new(OpenExplorer(HostId(DB_01))), cx);
    });
    in_frame(cx, main, |window, _| {
        assert!(window.find(("explorer", SFTP_TAB)).visible());
    });
    in_frame(cx, manager, |window, cx| {
        window.click("host-search", cx);
        window.dispatch_action(Box::new(CloseHostManager), cx);
    });
    assert!(
        cx.update(|cx| workspace.read(cx).host_manager_window())
            .is_none()
    );
    cx.update(|cx| {
        assert!(workspace.read(cx).terminal(HostId(DB_01), cx).is_some());
        assert!(workspace.read(cx).explorer(ExplorerId(SFTP_TAB)).is_some());
    });
    let reopened = open_host_manager(cx, main, &workspace);
    assert_ne!(reopened, manager);
    in_frame(cx, reopened, |window, _| {
        assert!(window.find(("host-row", created.0)).visible())
    });
}

#[gpui_kit::test]
fn host_manager_opens_from_an_empty_workspace_and_closes_with_its_owner(cx: &mut TestAppContext) {
    let (main, workspace) = open_workspace_with_store(cx, HostStore::empty());
    in_frame(cx, main, |window, cx| {
        let settings = window.find("open-settings").bounds();
        let hosts = window.find("show-hosts").bounds();
        assert_eq!(settings.top(), hosts.top());
        assert_eq!(settings.size, hosts.size);
        assert!(hosts.left() >= settings.right());
        window.click("show-hosts", cx)
    });
    let manager = cx
        .update(|cx| workspace.read(cx).host_manager_window())
        .unwrap();
    cx.update_window(manager, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("new-host-panel").visible());
        assert!(window.find("host-search").visible());
    })
    .unwrap();
    cx.update_window(main.into(), |_, window, _| window.remove_window())
        .unwrap();
    drop(workspace);
    cx.run_until_parked();
    assert!(cx.update_window(manager, |_, _, _| ()).is_err());
}

#[gpui_kit::test]
fn host_manager_fits_its_minimum_size_with_english_and_larger_text(cx: &mut TestAppContext) {
    let (main, workspace) = open_workspace_with_store(cx, HostStore::seed());
    let manager = open_host_manager(cx, main, &workspace);
    cx.simulate_window_resize(manager.into(), size(px(480.), px(360.)));
    cx.update(|cx| {
        workspace
            .read(cx)
            .settings()
            .clone()
            .update(cx, |settings, cx| {
                settings.update(
                    |settings| {
                        settings.language = InterfaceLanguage::English;
                        settings.interface.font_size = 20.;
                        settings.appearance = Appearance::Dark;
                    },
                    cx,
                );
            });
    });
    cx.run_until_parked();
    in_frame(cx, manager, |window, cx| {
        assert_eq!(
            window.find("host-search").label(),
            Some("Search hosts or groups")
        );
        let host = window.find("new-host-panel").bounds();
        let group = window.find("new-group").bounds();
        let more = window.find("host-manager-more").bounds();
        assert!(host.left() >= px(0.));
        assert!(host.right() <= group.left());
        assert!(group.right() <= more.left());
        assert!(more.right() <= window.viewport_size().width);
        window.press("secondary-n", cx);
    });
    in_frame(cx, manager, |window, cx| {
        assert!(window.find("host-name").visible());
        window.press("escape", cx);
    });
    for (ix, button, field) in [
        (1usize, "new-forward", "forward-name"),
        (2usize, "new-credential", "credential-name"),
    ] {
        in_frame(cx, manager, |window, cx| {
            window.within("manager-sections").click(ix, cx)
        });
        in_frame(cx, manager, |window, cx| {
            assert!(window.find(button).bounds().right() <= window.viewport_size().width);
            window.click(button, cx);
        });
        in_frame(cx, manager, |window, cx| {
            // Long forms scroll inside the dialog while its footer stays put.
            window.dispatch_event(
                gpui_kit::ScrollWheelEvent {
                    position: point(
                        window.viewport_size().width / 2.,
                        window.viewport_size().height / 2.,
                    ),
                    delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(10000.))),
                    modifiers: Default::default(),
                    touch_phase: gpui_kit::TouchPhase::Moved,
                }
                .to_platform_input(),
                cx,
            );
        });
        for _ in 0..20 {
            if in_frame(cx, manager, |window, _| window.find(field).visible()) {
                break;
            }
            in_frame(cx, manager, |window, cx| {
                window.dispatch_event(
                    gpui_kit::ScrollWheelEvent {
                        position: point(
                            window.viewport_size().width / 2.,
                            window.viewport_size().height / 2.,
                        ),
                        delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-40.))),
                        modifiers: Default::default(),
                        touch_phase: gpui_kit::TouchPhase::Moved,
                    }
                    .to_platform_input(),
                    cx,
                );
            });
        }
        in_frame(cx, manager, |window, cx| {
            assert!(window.find(field).visible());
            assert!(window.find("commit").visible());
            assert!(window.find("commit").bounds().bottom() <= window.viewport_size().height);
            window.press("escape", cx);
        });
    }
    assert!(
        cx.update(|cx| workspace.read(cx).host_manager_window())
            .is_some()
    );
    in_frame(cx, manager, |window, cx| {
        window.click("credential-search", cx);
        window.press("secondary-w", cx);
    });
    assert!(
        cx.update(|cx| workspace.read(cx).host_manager_window())
            .is_none()
    );
}

#[gpui_kit::test]
fn management_sections_keep_their_search_and_open_dialogs_in_the_same_window(
    cx: &mut TestAppContext,
) {
    let (main, workspace) = open_workspace(cx);
    let manager = open_host_manager(cx, main, &workspace);
    in_frame(cx, manager, |window, cx| {
        window.input("staging", cx);
        window.within("manager-sections").click(1usize, cx);
    });
    in_frame(cx, manager, |window, cx| {
        assert_eq!(window.find("forward-search").focused(), Some(true));
        assert!(window.try_find("host-search").is_none());
        window.input("database", cx);
        window.click("new-forward", cx);
    });
    in_frame(cx, manager, |window, cx| {
        assert!(window.find("forward-name").visible());
        window.press("escape", cx);
    });
    in_frame(cx, manager, |window, cx| {
        window.within("manager-sections").click(2usize, cx);
    });
    in_frame(cx, manager, |window, cx| {
        assert_eq!(window.find("credential-search").focused(), Some(true));
        window.input("deploy", cx);
        window.click("new-credential", cx);
    });
    in_frame(cx, manager, |window, cx| {
        assert!(window.find("credential-name").visible());
        window.press("escape", cx);
    });
    for (ix, search, value) in [
        (0usize, "host-search", "staging"),
        (1usize, "forward-search", "database"),
        (2usize, "credential-search", "deploy"),
    ] {
        in_frame(cx, manager, |window, cx| {
            window.within("manager-sections").click(ix, cx)
        });
        in_frame(cx, manager, |window, _| {
            assert_eq!(window.find(search).value(), Some(value));
        });
    }
    assert_eq!(
        cx.update(|cx| workspace.read(cx).host_manager_window()),
        Some(manager.into())
    );
    in_frame(cx, main, |window, _| {
        assert!(window.find("open-tabs").visible());
        assert!(window.try_find("forward-search").is_none());
        assert!(window.try_find("credential-search").is_none());
        assert!(window.try_find("forward-name").is_none());
        assert!(window.try_find("credential-name").is_none());
    });
}

#[gpui_kit::test]
async fn closing_management_keeps_running_forwards_and_cancels_unanswered_questions(
    cx: &mut TestAppContext,
) {
    for asks in [false, true] {
        let mut store = HostStore::seed();
        let id = store
            .insert_forward_unnotified(ForwardDraft::new(
                ForwardKind::Local,
                HostId(DB_01),
                ForwardEndpoint::new("127.0.0.1", 8080),
                Some(ForwardEndpoint::new("db.internal", 3306)),
            ))
            .unwrap();
        let provider = Arc::new(FakeForwardProvider::with_script(if asks {
            ForwardScript::AsksTrust
        } else {
            ForwardScript::Listens
        }));
        let (main, workspace) = open_workspace_with_forwards(cx, store, provider.clone());
        let manager = show_forwards(cx, main).await;
        in_frame(cx, manager, |window, cx| {
            window.click(("forward-toggle", id.0), cx)
        });
        cx.wait_for(manager.into(), Duration::from_secs(2), |window, _| {
            if asks {
                window.try_find("cancel").is_some()
            } else {
                window.find(("forward-status", id.0)).label() == Some("运行中")
            }
        })
        .await;
        in_frame(cx, main, |window, _| {
            assert!(window.try_find("cancel").is_none())
        });
        in_frame(cx, manager, |window, _| window.remove_window());
        cx.wait_for(main.into(), Duration::from_secs(2), |_, cx| {
            workspace.read(cx).host_manager_window().is_none() && (!asks || provider.stopped() == 1)
        })
        .await;
        if !asks {
            assert_eq!(provider.stopped(), 0);
            let reopened = show_forwards(cx, main).await;
            assert_ne!(reopened, manager);
            in_frame(cx, reopened, |window, _| {
                assert_eq!(
                    window.find(("forward-status", id.0)).label(),
                    Some("运行中")
                );
            });
        }
    }
}

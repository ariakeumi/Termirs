use crate::support::*;
use gpui_kit::component::dock::PanelId;

#[gpui_kit::test]
fn tab_categories_follow_host_group_paths_order_and_live_changes(cx: &mut TestAppContext) {
    use shellrs::host::{HostNode, NodeDrop};

    let (handle, workspace) = open_workspace(cx);
    let store = cx.update(|cx| workspace.read(cx).store().clone());
    let (production, testing) = cx.update(|cx| {
        store.update(cx, |store, cx| {
            let production =
                store.insert_group(GroupDraft::new("数据库", Some(GroupId(PRODUCTION))), cx);
            let testing = store.insert_group(GroupDraft::new("数据库", Some(GroupId(2))), cx);
            store.move_node(
                HostNode::Host(HostId(WEB_01)),
                NodeDrop::Into(production),
                cx,
            );
            store.move_node(
                HostNode::Host(HostId(STAGING_API)),
                NodeDrop::Into(testing),
                cx,
            );
            (production, testing)
        })
    });
    in_frame(cx, handle, |window, _| {
        for (id, label) in [(production, "生产 / 数据库"), (testing, "测试 / 数据库")] {
            assert_eq!(
                window.find(("tab-category-group", id.0)).label(),
                Some(label)
            );
            assert_eq!(
                window
                    .within(("tab-category-group", id.0))
                    .find("count")
                    .label(),
                Some("1")
            );
        }
        assert!(
            window
                .try_find(("tab-category-group", PRODUCTION))
                .is_none()
        );
        assert!(window.try_find("tab-category-ssh").is_none());
    });
    cx.update(|cx| {
        store.update(cx, |store, cx| {
            store.update_group(GroupId(PRODUCTION), GroupDraft::new("运营", None), cx);
            store.move_node(
                HostNode::Group(GroupId(PRODUCTION)),
                NodeDrop::After(HostNode::Group(GroupId(2))),
                cx,
            );
        });
    });
    in_frame(cx, handle, |window, cx| {
        let production = window.find(("tab-category-group", production.0));
        let testing = window.find(("tab-category-group", testing.0));
        assert_eq!(production.label(), Some("运营 / 数据库"));
        assert!(testing.bounds().bottom() < production.bounds().top());
        window.press(&tab_switch_stroke(1).unparse(), cx);
    });
    cx.update(|cx| {
        assert_eq!(
            workspace.read(cx).active_tab(),
            Some(CenterTab::Terminal(RemoteTerminalId(
                INITIAL_STAGING_TERMINAL
            )))
        )
    });
    cx.update(|cx| {
        store.update(cx, |store, cx| {
            store.move_node(
                HostNode::Host(HostId(STAGING_API)),
                NodeDrop::Into(production),
                cx,
            );
        });
    });
    in_frame(cx, handle, |window, _| {
        assert!(window.try_find(("tab-category-group", testing.0)).is_none());
        assert_eq!(
            window
                .within(("tab-category-group", production.0))
                .find("count")
                .label(),
            Some("2")
        );
    });
    cx.update(|cx| {
        store.update(cx, |store, cx| {
            store.move_node(HostNode::Host(HostId(WEB_01)), NodeDrop::Root, cx);
            store.remove(HostId(STAGING_API), cx);
        });
    });
    in_frame(cx, handle, |window, _| {
        assert_eq!(
            window.find("tab-category-ungrouped").label(),
            Some("未分组")
        );
        assert_eq!(
            window
                .within("tab-category-ungrouped")
                .find("count")
                .label(),
            Some("2")
        );
        assert!(
            window
                .try_find(("tab-category-group", production.0))
                .is_none()
        );
        assert!(
            window
                .find(("terminal-tab", INITIAL_WEB_TERMINAL))
                .visible()
        );
        assert!(
            window
                .find(("terminal-tab", INITIAL_STAGING_TERMINAL))
                .visible()
        );
    });
    cx.update(|cx| {
        workspace
            .read(cx)
            .settings()
            .clone()
            .update(cx, |settings, cx| {
                settings.update(
                    |settings| settings.language = InterfaceLanguage::English,
                    cx,
                );
            });
    });
    in_frame(cx, handle, |window, _| {
        assert_eq!(
            window.find("tab-category-ungrouped").label(),
            Some("Ungrouped")
        );
    });
}

#[gpui_kit::test]
fn tab_categories_group_connections_and_update_their_counts(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace(cx);
    in_frame(cx, handle, |window, cx| {
        assert!(window.try_find("tab-category-ssh").is_none());
        for (id, name) in [(PRODUCTION, "生产"), (2, "测试")] {
            assert_eq!(window.find(("tab-category-group", id)).label(), Some(name));
            assert_eq!(
                window
                    .within(("tab-category-group", id))
                    .find("count")
                    .label(),
                Some("1")
            );
        }
        assert!(window.try_find("tab-category-local").is_none());
        assert!(window.try_find("tab-category-sftp").is_none());
        window.dispatch_action(Box::new(NewLocalTerminal), cx);
    });
    in_frame(cx, handle, |window, cx| {
        window.dispatch_action(Box::new(OpenExplorer(HostId(DB_01))), cx);
    });
    in_frame(cx, handle, |window, cx| {
        window.dispatch_action(Box::new(NewLocalTerminal), cx);
    });
    for (appearance, font_size) in [(Appearance::Light, 16.), (Appearance::Dark, 20.)] {
        cx.update(|cx| {
            workspace
                .read(cx)
                .settings()
                .clone()
                .update(cx, |store, cx| {
                    store.update(
                        |settings| {
                            settings.appearance = appearance;
                            settings.interface.font_size = font_size;
                        },
                        cx,
                    );
                });
        });
        in_frame(cx, handle, |window, _| {
            let headings: [ElementId; 4] = [
                "tab-category-local".into(),
                ("tab-category-group", PRODUCTION).into(),
                ("tab-category-group", 2u64).into(),
                "tab-category-sftp".into(),
            ];
            for (id, title, count) in [
                (&headings[0], "Local", "2"),
                (&headings[1], "生产", "1"),
                (&headings[2], "测试", "1"),
                (&headings[3], "SFTP", "1"),
            ] {
                assert_eq!(window.find(id.clone()).label(), Some(title));
                assert_eq!(window.within(id.clone()).find("count").label(), Some(count));
            }
            let titles: [ElementId; 5] = [
                ("local-terminal-tab", 1u64).into(),
                ("local-terminal-tab", 2u64).into(),
                ("terminal-tab", INITIAL_WEB_TERMINAL).into(),
                ("terminal-tab", INITIAL_STAGING_TERMINAL).into(),
                ("explorer-tab", SFTP_TAB).into(),
            ];
            for pair in titles.windows(2) {
                assert!(
                    window.find(pair[0].clone()).bounds().bottom()
                        <= window.find(pair[1].clone()).bounds().top()
                );
            }
            let inset = window.rem_size();
            for (heading, first) in [
                (&headings[0], &titles[0]),
                (&headings[1], &titles[2]),
                (&headings[2], &titles[3]),
                (&headings[3], &titles[4]),
            ] {
                let heading = window.find(heading.clone()).bounds();
                let title = window.find(first.clone()).bounds();
                assert_eq!(heading.left(), title.left());
                assert!(heading.bottom() < title.top());
                assert!(heading.size.height < inset * 2.);
            }
        });
    }
    in_frame(cx, handle, |window, cx| {
        window.dispatch_action(
            Box::new(shellrs::app::CloseLocalTerminal(LocalTerminalId(1))),
            cx,
        );
    });
    in_frame(cx, handle, |window, cx| {
        assert_eq!(
            window.within("tab-category-local").find("count").label(),
            Some("1")
        );
        window.dispatch_action(
            Box::new(shellrs::app::CloseLocalTerminal(LocalTerminalId(2))),
            cx,
        );
    });
    in_frame(cx, handle, |window, cx| {
        assert!(window.try_find("tab-category-local").is_none());
        window.dispatch_action(
            Box::new(shellrs::app::CloseExplorer(ExplorerId(SFTP_TAB))),
            cx,
        );
    });
    in_frame(cx, handle, |window, _| {
        assert!(window.try_find("tab-category-sftp").is_none());
        assert_eq!(
            window
                .within(("tab-category-group", PRODUCTION))
                .find("count")
                .label(),
            Some("1")
        );
    });
}

#[gpui_kit::test]
fn tab_categories_share_visual_order_with_shortcuts_and_arrow_navigation(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace(cx);
    in_frame(cx, handle, |window, cx| {
        window.dispatch_action(Box::new(OpenExplorer(HostId(DB_01))), cx);
    });
    in_frame(cx, handle, |window, cx| {
        window.dispatch_action(Box::new(NewLocalTerminal), cx);
    });
    in_frame(cx, handle, |window, cx| {
        window.click("open-settings", cx);
    });
    let local_row: ElementId = cx.update(|cx| {
        let panel = workspace
            .read(cx)
            .local_terminal(LocalTerminalId(1))
            .unwrap();
        ("open-tab", PanelId::from(panel.entity_id()).as_u64()).into()
    });
    let web = terminal_row(&workspace, INITIAL_WEB_TERMINAL, cx);
    hold_tab_modifiers(cx, handle, tab_switch_stroke(1).modifiers);
    cx.executor().advance_clock(Duration::from_millis(500));
    in_frame(cx, handle, |window, cx| {
        for (row, digit) in [(local_row, 1), (web, 2)] {
            assert_eq!(
                window.within(row).find("tab-shortcut-hint").label(),
                Some(gpui_kit::component::kbd::Kbd::format(&tab_switch_stroke(digit)).as_str())
            );
        }
        window.press(&tab_switch_stroke(1).unparse(), cx);
    });
    cx.update(|cx| {
        assert_eq!(
            workspace.read(cx).active_tab(),
            Some(CenterTab::LocalTerminal(LocalTerminalId(1)))
        )
    });
    in_frame(cx, handle, |window, cx| window.press("ctrl-tab", cx));
    cx.update(|cx| {
        assert_eq!(
            workspace.read(cx).active_tab(),
            Some(CenterTab::Terminal(RemoteTerminalId(INITIAL_WEB_TERMINAL)))
        )
    });
    in_frame(cx, handle, |window, cx| window.press("ctrl-shift-tab", cx));
    cx.update(|cx| {
        assert_eq!(
            workspace.read(cx).active_tab(),
            Some(CenterTab::LocalTerminal(LocalTerminalId(1)))
        )
    });
    in_frame(cx, handle, |window, cx| {
        window.dispatch_action(Box::new(shellrs::app::ShowTabs), cx);
    });
    in_frame(cx, handle, |window, cx| window.press("down", cx));
    in_frame(cx, handle, |window, _| {
        assert_eq!(window.find("open-tabs").focused(), Some(true));
        assert!(window.find(("terminal", INITIAL_WEB_TERMINAL)).visible());
    });
    in_frame(cx, handle, |window, cx| {
        window.press(&tab_switch_stroke(4).unparse(), cx)
    });
    cx.update(|cx| {
        assert_eq!(
            workspace.read(cx).active_tab(),
            Some(CenterTab::Explorer(ExplorerId(SFTP_TAB)))
        )
    });
    in_frame(cx, handle, |window, cx| {
        window.press(&tab_switch_stroke(9).unparse(), cx)
    });
    cx.update(|cx| assert_eq!(workspace.read(cx).active_tab(), Some(CenterTab::Settings)));
}

#[gpui_kit::test]
fn tab_categories_hide_when_empty_and_scroll_to_tabs_after_headings(cx: &mut TestAppContext) {
    let (handle, _) = open_workspace_with_store(cx, HostStore::empty());
    in_frame(cx, handle, |window, _| {
        for id in [
            "tab-category-local",
            "tab-category-ssh",
            "tab-category-sftp",
        ] {
            assert!(window.try_find(id).is_none());
        }
    });
    for _ in 0..20 {
        in_frame(cx, handle, |window, cx| {
            window.dispatch_action(Box::new(NewLocalTerminal), cx)
        });
    }
    in_frame(cx, handle, |window, cx| window.click("open-settings", cx));
    in_frame(cx, handle, |window, cx| {
        assert!(window.find("settings-tab").visible());
        window.press(&tab_switch_stroke(1).unparse(), cx);
    });
    in_frame(cx, handle, |window, cx| {
        assert!(window.find(("local-terminal-tab", 1u64)).visible());
        assert!(!window.find("settings-tab").visible());
        window.press(&tab_switch_stroke(9).unparse(), cx);
    });
    in_frame(cx, handle, |window, _| {
        assert!(window.find("settings-tab").visible())
    });
}

fn tab_switch_stroke(digit: usize) -> gpui_kit::Keystroke {
    let mut stroke = gpui_kit::Keystroke::parse(
        shellrs::app::Shortcut::find("switch-to-tab")
            .unwrap()
            .default_keys(),
    )
    .unwrap();
    stroke.key = digit.to_string();
    stroke
}

fn hold_tab_modifiers(
    cx: &mut TestAppContext,
    handle: WindowHandle<Root>,
    modifiers: gpui_kit::Modifiers,
) {
    in_frame(cx, handle, |window, cx| {
        window.dispatch_event(
            gpui_kit::PlatformInput::ModifiersChanged(gpui_kit::ModifiersChangedEvent {
                modifiers,
                ..Default::default()
            }),
            cx,
        );
    });
}

#[gpui_kit::test]
fn tab_shortcut_hints_wait_for_a_hold_and_hide_on_release(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace(cx);
    let web = terminal_row(&workspace, INITIAL_WEB_TERMINAL, cx);
    let staging = terminal_row(&workspace, INITIAL_STAGING_TERMINAL, cx);
    in_frame(cx, handle, |window, cx| {
        window.click(("terminal-tab", INITIAL_WEB_TERMINAL), cx);
    });
    let before = in_frame(cx, handle, |window, _| window.find(web.clone()).bounds());
    let modifiers = tab_switch_stroke(1).modifiers;
    hold_tab_modifiers(cx, handle, modifiers);
    cx.executor().advance_clock(Duration::from_millis(499));
    in_frame(cx, handle, |window, _| {
        assert!(
            window
                .within(web.clone())
                .try_find("tab-shortcut-hint")
                .is_none()
        );
    });
    hold_tab_modifiers(cx, handle, Default::default());
    cx.executor().advance_clock(Duration::from_millis(600));
    in_frame(cx, handle, |window, _| {
        assert!(
            window
                .within(web.clone())
                .try_find("tab-shortcut-hint")
                .is_none()
        );
    });
    hold_tab_modifiers(cx, handle, modifiers);
    cx.executor().advance_clock(Duration::from_millis(500));
    in_frame(cx, handle, |window, _| {
        for (row, digit) in [(web.clone(), 1), (staging.clone(), 2)] {
            let hint = window.within(row.clone()).find("tab-shortcut-hint");
            assert!(hint.visible());
            assert_eq!(
                hint.label(),
                Some(gpui_kit::component::kbd::Kbd::format(&tab_switch_stroke(digit)).as_str())
            );
            let label = window.within(row).find("label");
            assert!(label.bounds().right() <= hint.bounds().left());
        }
        assert_eq!(window.find(web.clone()).bounds(), before);
    });
    hold_tab_modifiers(cx, handle, Default::default());
    in_frame(cx, handle, |window, _| {
        assert!(
            window
                .within(web.clone())
                .try_find("tab-shortcut-hint")
                .is_none()
        );
    });
}

#[gpui_kit::test]
fn tab_shortcut_hints_cancel_on_deactivation_and_follow_custom_keys(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace(cx);
    let web = terminal_row(&workspace, INITIAL_WEB_TERMINAL, cx);
    in_frame(cx, handle, |window, cx| {
        window.activate_window();
        window.click(("terminal-tab", INITIAL_WEB_TERMINAL), cx);
    });
    hold_tab_modifiers(cx, handle, tab_switch_stroke(1).modifiers);
    gpui_kit::VisualTestContext::from_window(handle.into(), cx).deactivate_window();
    cx.executor().advance_clock(Duration::from_millis(600));
    in_frame(cx, handle, |window, _| {
        assert!(
            window
                .within(web.clone())
                .try_find("tab-shortcut-hint")
                .is_none()
        );
        window.activate_window();
    });
    let shortcut = shellrs::app::Shortcut::find("switch-to-tab").unwrap();
    cx.update(|cx| {
        workspace
            .read(cx)
            .settings()
            .clone()
            .update(cx, |settings, cx| {
                settings.update(
                    |settings| settings.shortcuts.set(shortcut, "ctrl-alt-1".into()),
                    cx,
                );
            });
    });
    hold_tab_modifiers(cx, handle, tab_switch_stroke(1).modifiers);
    cx.executor().advance_clock(Duration::from_millis(600));
    in_frame(cx, handle, |window, _| {
        assert!(
            window
                .within(web.clone())
                .try_find("tab-shortcut-hint")
                .is_none()
        );
    });
    let custom = gpui_kit::Keystroke::parse("ctrl-alt-1").unwrap();
    hold_tab_modifiers(cx, handle, custom.modifiers);
    cx.executor().advance_clock(Duration::from_millis(500));
    in_frame(cx, handle, |window, _| {
        assert_eq!(
            window.within(web.clone()).find("tab-shortcut-hint").label(),
            Some(gpui_kit::component::kbd::Kbd::format(&custom).as_str())
        );
    });
    gpui_kit::VisualTestContext::from_window(handle.into(), cx).deactivate_window();
    in_frame(cx, handle, |window, _| {
        assert!(
            window
                .within(web.clone())
                .try_find("tab-shortcut-hint")
                .is_none()
        );
        window.activate_window();
    });
    cx.update(|cx| {
        workspace
            .read(cx)
            .settings()
            .clone()
            .update(cx, |settings, cx| {
                settings.update(|settings| settings.shortcuts.disable(shortcut), cx);
            });
    });
    hold_tab_modifiers(cx, handle, Default::default());
    hold_tab_modifiers(cx, handle, custom.modifiers);
    cx.executor().advance_clock(Duration::from_millis(600));
    in_frame(cx, handle, |window, _| {
        assert!(
            window
                .within(web.clone())
                .try_find("tab-shortcut-hint")
                .is_none()
        );
    });
}

#[gpui_kit::test]
fn tab_shortcut_hints_use_nine_for_the_last_tab_and_renumber_after_closing(
    cx: &mut TestAppContext,
) {
    let (handle, workspace) = open_workspace_with_store(cx, HostStore::empty());
    for _ in 0..10 {
        in_frame(cx, handle, |window, cx| {
            window.dispatch_action(Box::new(shellrs::app::NewLocalTerminal), cx);
        });
    }
    let rows: Vec<ElementId> = cx.update(|cx| {
        (1..=10)
            .map(|id| {
                let panel = workspace
                    .read(cx)
                    .local_terminal(shellrs::terminal::LocalTerminalId(id))
                    .unwrap();
                ("open-tab", PanelId::from(panel.entity_id()).as_u64()).into()
            })
            .collect()
    });
    hold_tab_modifiers(cx, handle, tab_switch_stroke(1).modifiers);
    cx.executor().advance_clock(Duration::from_millis(500));
    in_frame(cx, handle, |window, cx| {
        for (ix, row) in rows.iter().enumerate() {
            if ix == 8 {
                assert!(
                    window
                        .within(row.clone())
                        .try_find("tab-shortcut-hint")
                        .is_none()
                );
            } else {
                let digit = if ix == 9 { 9 } else { ix + 1 };
                assert_eq!(
                    window.within(row.clone()).find("tab-shortcut-hint").label(),
                    Some(gpui_kit::component::kbd::Kbd::format(&tab_switch_stroke(digit)).as_str())
                );
            }
        }
        window.press(&tab_switch_stroke(1).unparse(), cx);
    });
    cx.update(|cx| {
        assert_eq!(
            workspace.read(cx).active_tab(),
            Some(CenterTab::LocalTerminal(
                shellrs::terminal::LocalTerminalId(1)
            ))
        )
    });
    in_frame(cx, handle, |window, cx| {
        window.press(&tab_switch_stroke(9).unparse(), cx);
    });
    cx.update(|cx| {
        assert_eq!(
            workspace.read(cx).active_tab(),
            Some(CenterTab::LocalTerminal(
                shellrs::terminal::LocalTerminalId(10)
            ))
        )
    });
    hold_tab_modifiers(cx, handle, Default::default());
    hold_tab_modifiers(cx, handle, tab_switch_stroke(1).modifiers);
    cx.executor().advance_clock(Duration::from_millis(500));
    in_frame(cx, handle, |window, cx| {
        window.dispatch_action(
            Box::new(shellrs::app::CloseLocalTerminal(
                shellrs::terminal::LocalTerminalId(1),
            )),
            cx,
        );
    });
    in_frame(cx, handle, |window, _| {
        for (ix, row) in rows.iter().skip(1).enumerate() {
            assert_eq!(
                window.within(row.clone()).find("tab-shortcut-hint").label(),
                Some(gpui_kit::component::kbd::Kbd::format(&tab_switch_stroke(ix + 1)).as_str())
            );
        }
    });
}

#[gpui_kit::test]
fn sidebar_tab_text_has_its_own_font_size(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace_with_store(cx, HostStore::empty());
    in_frame(cx, handle, |window, cx| {
        window.dispatch_action(Box::new(shellrs::app::NewLocalTerminal), cx);
    });
    let label_height = |cx: &mut TestAppContext| {
        in_frame(cx, handle, |window, _| {
            window
                .within(("local-terminal-tab", 1u64))
                .find("label")
                .bounds()
                .size
                .height
        })
    };
    let original = label_height(cx);
    let settings = cx.update(|cx| workspace.read(cx).settings().clone());
    cx.update(|cx| {
        settings.update(cx, |settings, cx| {
            settings.update(|settings| settings.interface.sidebar_font_size = 20., cx);
        });
    });
    cx.run_until_parked();
    let larger = label_height(cx);
    assert!(larger > original, "{original:?} → {larger:?}");
    cx.update(|cx| {
        settings.update(cx, |settings, cx| {
            settings.update(|settings| settings.interface.font_size = 12., cx);
        });
    });
    cx.run_until_parked();
    assert_eq!(label_height(cx), larger);
}

#[gpui_kit::test]
fn the_tab_sidebar_can_be_dragged_wider_and_narrower(cx: &mut TestAppContext) {
    let (handle, _) = open_workspace(cx);
    let bounds = |cx: &mut TestAppContext| {
        in_frame(cx, handle, |window, _| {
            window.find("workspace-sidebar").bounds()
        })
    };
    let opened = bounds(cx);
    drag_sidebar(
        cx,
        handle,
        point(opened.right() - px(2.), opened.center().y),
        100.,
    );
    let wider = bounds(cx);
    assert!(
        (wider.size.width - opened.size.width - gpui_kit::px(100.)).abs() < gpui_kit::px(3.),
        "{opened:?} → {wider:?}"
    );
    drag_sidebar(
        cx,
        handle,
        point(wider.right() - px(2.), wider.center().y),
        -100.,
    );
    let narrowed = bounds(cx);
    assert!(
        (narrowed.size.width - opened.size.width).abs() < gpui_kit::px(5.),
        "{opened:?} → {narrowed:?}"
    );
}

fn drag_sidebar(
    cx: &mut TestAppContext,
    handle: WindowHandle<Root>,
    from: gpui_kit::Point<gpui_kit::Pixels>,
    by: f32,
) {
    let to = point(from.x + px(by), from.y);
    in_frame(cx, handle, |window, cx| {
        window.dispatch_event(
            gpui_kit::PlatformInput::MouseDown(gpui_kit::MouseDownEvent {
                button: MouseButton::Left,
                position: from,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        for position in [point(from.x + px(by.signum() * 6.), from.y), to] {
            window.dispatch_event(
                gpui_kit::PlatformInput::MouseMove(MouseMoveEvent {
                    position,
                    pressed_button: Some(MouseButton::Left),
                    modifiers: Default::default(),
                }),
                cx,
            );
        }
        window.dispatch_event(
            gpui_kit::PlatformInput::MouseUp(gpui_kit::MouseUpEvent {
                button: MouseButton::Left,
                position: to,
                modifiers: Default::default(),
                click_count: 1,
            }),
            cx,
        );
    });
}

/// A fresh workspace with settings loaded from disk, as on another launch.
fn reopen_with_settings(
    cx: &mut TestAppContext,
    path: &std::path::Path,
) -> (WindowHandle<Root>, Entity<Workspace>) {
    let (mut settings, problem) = SettingsStore::load(path.to_path_buf());
    assert!(problem.is_none());
    settings.update_unnotified(|settings| settings.language = InterfaceLanguage::SimplifiedChinese);
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(800.)), |window, cx| {
        let view = cx.new(|cx| {
            Workspace::new_with_services(
                cx.new(|_| HostStore::seed()),
                cx.new(|_| settings),
                None,
                Arc::new(FixedRemoteTerminalTransportProvider::new(Arc::new(
                    FakeTerminalFactory::default(),
                ))),
                fake_telnet_terminals(),
                fake_serial_terminals(),
                fake_local_terminals(),
                Arc::new(FakeSftpProvider::default()),
                Arc::new(FakeLocalDirectory::default()),
                Arc::new(FakeConnectionTester::default()),
                Arc::new(FakeForwardProvider::default()),
                window,
                cx,
            )
        });
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    (handle, workspace.unwrap())
}

#[gpui_kit::test]
fn sidebar_widths_are_saved_and_restored_after_reopening(cx: &mut TestAppContext) {
    init_app(cx);
    cx.update(|cx| cx.set_reduce_motion(true));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let (handle, workspace) = reopen_with_settings(cx, &path);
    let left = in_frame(cx, handle, |window, _| {
        window.find("workspace-sidebar").bounds()
    });
    drag_sidebar(
        cx,
        handle,
        point(left.right() - px(2.), left.center().y),
        100.,
    );
    in_frame(cx, handle, |window, cx| {
        window.click(("terminal-tab", INITIAL_WEB_TERMINAL), cx);
    });
    in_frame(cx, handle, |window, cx| {
        toggle_tool(window, "tool-snippets", cx)
    });
    let right = in_frame(cx, handle, |window, _| {
        window.find(("tool-sidebar", INITIAL_WEB_TERMINAL)).bounds()
    });
    drag_sidebar(
        cx,
        handle,
        point(right.left() + px(2.), right.center().y),
        -80.,
    );
    let widths = cx.update(|cx| {
        workspace
            .read(cx)
            .settings()
            .read(cx)
            .settings()
            .sidebar_widths
    });
    assert!((widths.left() - 380.).abs() < 3.);
    assert!((widths.right() - 400.).abs() < 3.);
    // Collapsing either sidebar must retain the last chosen widths.
    in_frame(cx, handle, |window, cx| {
        toggle_tool(window, "tool-snippets", cx)
    });
    in_frame(cx, handle, |window, cx| {
        window.dispatch_action(Box::new(ToggleHostPanel), cx);
    });
    let (loaded, problem) = SettingsStore::load(path.clone());
    assert!(problem.is_none());
    assert_eq!(loaded.settings().sidebar_widths, widths);
    in_frame(cx, handle, |window, _| window.remove_window());
    drop(workspace);
    cx.run_until_parked();

    let (handle, _) = reopen_with_settings(cx, &path);
    in_frame(cx, handle, |window, cx| {
        let restored = window.find("workspace-sidebar").bounds().size.width;
        assert!((restored - px(widths.left())).abs() < px(1.));
        window.click(("terminal-tab", INITIAL_WEB_TERMINAL), cx);
    });
    in_frame(cx, handle, |window, cx| {
        toggle_tool(window, "tool-snippets", cx)
    });
    in_frame(cx, handle, |window, _| {
        let restored = window
            .find(("tool-sidebar", INITIAL_WEB_TERMINAL))
            .bounds()
            .size
            .width;
        assert!((restored - px(widths.right())).abs() < px(1.));
    });
}

fn terminal_row(
    workspace: &Entity<Workspace>,
    terminal: u64,
    cx: &mut TestAppContext,
) -> ElementId {
    cx.update(|cx| {
        let panel = workspace
            .read(cx)
            .remote_terminal(RemoteTerminalId(terminal))
            .unwrap();
        ("open-tab", PanelId::from(panel.entity_id()).as_u64()).into()
    })
}

#[gpui_kit::test]
fn open_tabs_are_vertical_and_switch_the_content_on_the_right(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace(cx);
    let web = terminal_row(&workspace, INITIAL_WEB_TERMINAL, cx);
    let staging = terminal_row(&workspace, INITIAL_STAGING_TERMINAL, cx);
    in_frame(cx, handle, |window, cx| {
        assert!(window.find("open-tabs").visible());
        for id in [
            "show-hosts",
            "show-tabs",
            "show-forwards",
            "show-credentials",
            "temporary-connection",
            "new-local-terminal",
            "theme-toggle",
            "toggle-hosts",
            "new-tab-sidebar",
        ] {
            assert!(
                window.try_find(id).is_none(),
                "removed chrome control: {id}"
            );
        }
        assert!(
            window
                .try_find(("close-terminal", INITIAL_WEB_TERMINAL))
                .is_none()
        );
        let sidebar = window.find("workspace-sidebar").bounds();
        let list = window.find("open-tabs").bounds();
        let settings = window.find("open-settings").bounds();
        assert_eq!(sidebar.top(), list.top());
        assert!(window.try_find("center-toolbar").is_none());
        let terminal = window.find(("terminal", INITIAL_WEB_TERMINAL)).bounds();
        if cfg!(target_os = "macos") {
            let content = window
                .within("dock-area-center")
                .find("active-panel")
                .bounds();
            assert_eq!(content.top(), window.find("workspace").bounds().top());
            assert!(list.top() > terminal.top());
        }
        assert!(settings.top() >= list.bottom());
        assert!(settings.bottom() <= sidebar.bottom());
        let first = window.find(web.clone()).bounds();
        let second = window.find(staging.clone()).bounds();
        assert_eq!(first.left(), second.left());
        assert_eq!(first.right(), second.right());
        assert!(first.bottom() <= second.top());
        assert!(
            window
                .within("dock-area-center")
                .try_find(("terminal-tab", INITIAL_WEB_TERMINAL))
                .is_none()
        );
        window.click(("terminal-tab", INITIAL_STAGING_TERMINAL), cx);
    });
    in_frame(cx, handle, |window, _| {
        assert_eq!(window.find(staging.clone()).selected(), Some(true));
        assert_eq!(window.find(web.clone()).selected(), Some(false));
        let content = window.find(("terminal", INITIAL_STAGING_TERMINAL)).bounds();
        assert!(content.left() >= window.find("open-tabs").bounds().right());
        assert!(
            window
                .try_find(("terminal", INITIAL_WEB_TERMINAL))
                .is_none()
        );
    });
    cx.update(|cx| {
        assert_eq!(
            workspace.read(cx).active_tab(),
            Some(CenterTab::Terminal(RemoteTerminalId(
                INITIAL_STAGING_TERMINAL
            )))
        )
    });
}

#[gpui_kit::test]
fn open_tabs_support_keyboard_navigation_and_closing_the_last_tab(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace(cx);
    in_frame(cx, handle, |window, cx| {
        window.click(("terminal-tab", INITIAL_WEB_TERMINAL), cx);
    });
    in_frame(cx, handle, |window, cx| {
        window.dispatch_action(Box::new(shellrs::app::ShowTabs), cx)
    });
    in_frame(cx, handle, |window, cx| {
        assert_eq!(window.find("open-tabs").focused(), Some(true));
        window.press("down", cx);
    });
    in_frame(cx, handle, |window, cx| {
        assert_eq!(window.find("open-tabs").focused(), Some(true));
        assert!(
            window
                .find(("terminal", INITIAL_STAGING_TERMINAL))
                .visible()
        );
        window.press("enter", cx);
    });
    in_frame(cx, handle, |window, cx| {
        assert_eq!(
            window
                .find(("terminal", INITIAL_STAGING_TERMINAL))
                .focused(),
            Some(true)
        );
        window.press("secondary-w", cx);
    });
    in_frame(cx, handle, |window, cx| {
        assert!(
            window
                .try_find(("terminal-tab", INITIAL_STAGING_TERMINAL))
                .is_none()
        );
        window.dispatch_action(
            Box::new(shellrs::app::CloseTerminal(RemoteTerminalId(
                INITIAL_WEB_TERMINAL,
            ))),
            cx,
        );
    });
    in_frame(cx, handle, |window, _| {
        assert!(window.find("recent-hosts").visible());
        assert!(window.find("open-tabs").visible());
        assert!(
            window
                .try_find(("terminal-tab", INITIAL_WEB_TERMINAL))
                .is_none()
        );
    });
    cx.update(|cx| assert_eq!(workspace.read(cx).active_tab(), None));
}

#[gpui_kit::test]
fn open_tabs_keep_all_tab_types_and_truncate_long_titles(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace(cx);
    in_frame(cx, handle, |window, cx| {
        window.dispatch_action(Box::new(shellrs::app::NewLocalTerminal), cx)
    });
    in_frame(cx, handle, |window, cx| {
        assert!(
            window
                .within("open-tabs")
                .find(("local-terminal-tab", 1u64))
                .visible()
        );
        window.dispatch_action(Box::new(OpenExplorer(HostId(DB_01))), cx);
    });
    in_frame(cx, handle, |window, cx| {
        assert!(
            window
                .within("open-tabs")
                .find(("explorer-tab", SFTP_TAB))
                .visible()
        );
        window.click("open-settings", cx);
    });
    in_frame(cx, handle, |window, cx| {
        assert!(window.within("open-tabs").find("settings-tab").visible());
        window.click(("terminal-tab", INITIAL_WEB_TERMINAL), cx);
        window.dispatch_action(
            Box::new(RenameTerminal(RemoteTerminalId(INITIAL_WEB_TERMINAL))),
            cx,
        );
    });
    in_frame(cx, handle, |window, cx| {
        window.click("tab-name", cx);
        window.press("secondary-a", cx);
        window.input(
            "生产环境的一个非常长的终端标签名称，用于检查侧边栏中的省略显示",
            cx,
        );
        window.click("commit", cx);
    });
    in_frame(cx, handle, |window, _| {
        let title = window.find(("terminal-tab", INITIAL_WEB_TERMINAL)).bounds();
        let sidebar = window.find("open-tabs").bounds();
        assert!(title.left() >= sidebar.left());
        assert!(title.right() <= sidebar.right());
        assert!(
            window
                .find(("terminal-tab", INITIAL_WEB_TERMINAL))
                .visible()
        );
    });
    cx.update(|cx| assert!(workspace.read(cx).host_manager_window().is_none()));
}

#[gpui_kit::test]
fn tabs_that_fit_in_the_sidebar_do_not_scroll(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace_with_store(cx, HostStore::empty());
    for count in 1..=3u64 {
        in_frame(cx, handle, |window, cx| {
            window.dispatch_action(Box::new(shellrs::app::NewLocalTerminal), cx);
        });
        for font_size in [16., 20.] {
            cx.update(|cx| {
                let settings = workspace.read(cx).settings().clone();
                settings.update(cx, |store, cx| {
                    store.update(|settings| settings.interface.font_size = font_size, cx);
                });
            });
            cx.run_until_parked();
            let before = in_frame(cx, handle, |window, _| {
                assert!(window.find(("local-terminal-tab", count)).visible());
                assert_eq!(
                    window.find(("local-terminal-tab", count)).label(),
                    Some("-zsh")
                );
                window.find(("local-terminal-tab", 1u64)).bounds()
            });
            in_frame(cx, handle, |window, cx| {
                window.scroll(
                    "open-tabs",
                    gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-100.))),
                    cx,
                );
            });
            in_frame(cx, handle, |window, _| {
                assert_eq!(window.find(("local-terminal-tab", 1u64)).bounds(), before);
            });
        }
    }
}

#[gpui_kit::test]
fn open_tabs_scroll_to_the_active_tab_and_keep_their_own_text_size(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace_with_store(cx, HostStore::empty());
    for _ in 0..20 {
        in_frame(cx, handle, |window, cx| {
            window.dispatch_action(Box::new(shellrs::app::NewLocalTerminal), cx)
        });
    }
    in_frame(cx, handle, |window, cx| {
        assert!(window.find(("local-terminal-tab", 20u64)).visible());
        window.press("secondary-1", cx);
    });
    let original = in_frame(cx, handle, |window, _| {
        assert!(window.find(("local-terminal-tab", 1u64)).visible());
        assert!(!window.find(("local-terminal-tab", 20u64)).visible());
        window
            .find(("local-terminal-tab", 1u64))
            .bounds()
            .size
            .height
    });
    cx.update(|cx| {
        let settings = workspace.read(cx).settings().clone();
        settings.update(cx, |settings, cx| {
            settings.update(
                |settings| {
                    settings.interface.font_size = 20.;
                    settings.language = InterfaceLanguage::English;
                    settings.appearance = Appearance::Dark;
                },
                cx,
            )
        });
    });
    cx.run_until_parked();
    in_frame(cx, handle, |window, _| {
        let title = window.find(("local-terminal-tab", 1u64));
        assert!(title.visible());
        assert_eq!(title.bounds().size.height, original);
        assert!(title.bounds().right() <= window.find("open-tabs").bounds().right());
    });
}

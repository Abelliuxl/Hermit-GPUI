mod agent;
mod cache;
mod hermes_config;
mod jsonl_process;
mod keychain;
mod local_server;
mod logger;
mod markdown;
mod models;
mod settings;
mod state;
mod ui;

use gpui::prelude::*;
use gpui::{
    actions, point, px, size, App, Application, Bounds, KeyBinding, Menu, MenuItem, WindowBounds,
    WindowOptions,
};
use state::AppState;
use ui::root::RootView;
use ui::settings_window::OpenSettings;

actions!(hermit, [NewSession, RefreshSessions, Quit]);

struct StateGlobal(gpui::Entity<AppState>);
impl gpui::Global for StateGlobal {}

struct SettingsWindowGlobal(Option<gpui::WindowHandle<ui::settings_window::SettingsView>>);
impl gpui::Global for SettingsWindowGlobal {}

fn main() {
    // Dedicated async runtime for backend transports (HTTP, WS, SSE, CLIs).
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    Application::new().run(move |cx: &mut App| {
        cx.set_global(state::TokioGlobal(runtime));
        ui::editor::bind_editor_keys(cx);

        let bounds = Bounds::centered(None, size(px(1180.0), px(760.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("Hermit".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(12.0), px(12.0))),
                    }),
                    window_min_size: Some(size(px(560.0), px(480.0))),
                    ..Default::default()
                },
                |_window, cx| {
                    let state = cx.new(|cx| AppState::new(cx));
                    cx.set_global(StateGlobal(state.clone()));
                    cx.new(|cx| RootView::new(state, cx))
                },
            )
            .expect("failed to open main window");

        window
            .update(cx, |_root, window, cx| {
                window.activate_window();
                // Minimize instead of closing so state survives the red button,
                // mirroring the SwiftUI app's close-to-hide behavior.
                window.on_window_should_close(cx, |window, _cx| {
                    window.minimize_window();
                    false
                });
            })
            .ok();

        // App-level actions.
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &NewSession, cx| {
            let state = cx.global::<StateGlobal>().0.clone();
            state.update(cx, |state, cx| state.start_fresh_chat(cx));
        });
        cx.on_action(|_: &RefreshSessions, cx| {
            let state = cx.global::<StateGlobal>().0.clone();
            state.update(cx, |state, cx| state.refresh_sessions(true, cx));
        });
        cx.on_action(|_: &OpenSettings, cx| {
            let existing = cx
                .try_global::<SettingsWindowGlobal>()
                .and_then(|global| global.0.clone());
            if let Some(handle) = existing {
                let _ = handle.update(cx, |_view, window, _cx| window.activate_window());
            } else {
                let state = cx.global::<StateGlobal>().0.clone();
                match ui::settings_window::SettingsView::open(state, cx) {
                    Ok(handle) => {
                        handle
                            .update(cx, |_view, window, cx| {
                                window.activate_window();
                                window.on_window_should_close(cx, |window, _cx| {
                                    window.minimize_window();
                                    false
                                });
                            })
                            .ok();
                        cx.set_global(SettingsWindowGlobal(Some(handle)));
                    }
                    Err(error) => {
                        log_debug!("app", "failed to open settings window: {error}");
                    }
                }
            }
        });

        // Keyboard shortcuts for the app actions.
        cx.bind_keys([
            KeyBinding::new("cmd-n", NewSession, None),
            KeyBinding::new("cmd-r", RefreshSessions, None),
            KeyBinding::new("cmd-,", OpenSettings, None),
            KeyBinding::new("cmd-q", Quit, None),
        ]);

        // Native menu bar.
        cx.set_menus(vec![
            Menu {
                name: "Hermit".into(),
                items: vec![
                    MenuItem::action("About Hermit", OpenSettings),
                    MenuItem::separator(),
                    MenuItem::action("Settings", OpenSettings),
                    MenuItem::separator(),
                    MenuItem::action("Quit Hermit", Quit),
                ],
            },
            Menu {
                name: "File".into(),
                items: vec![MenuItem::action("New Session", NewSession)],
            },
            Menu {
                name: "View".into(),
                items: vec![MenuItem::action("Refresh Sessions", RefreshSessions)],
            },
        ]);

        cx.activate(true);
    });
}

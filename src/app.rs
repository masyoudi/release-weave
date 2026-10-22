use std::borrow::Cow;

use gpui_kit::assets::{Assets, icon_assets};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, AssetSource, Bounds, Point, Result, SharedString, TitlebarOptions, WindowBounds,
    WindowOptions, px, size,
};

use release_weave::paths;
use release_weave::ui::shell::ShellView;
use release_weave::ui::state::AppState;

icon_assets!(
    ExtraIcons,
    [
        Archive,
        Boxes,
        Bug,
        CheckCheck,
        Code,
        Download,
        FolderGit2,
        GitCommitHorizontal,
        Lock,
        LockOpen,
        Package,
        Plug,
        SaveCheck,
        ShieldCheck,
        Sparkles,
        SquareCheck,
        Tag,
        Trash,
        Upload,
        X,
    ]
);

struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = Assets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

pub fn run() {
    gpui_kit::application()
        .with_assets(AppAssets)
        .run(|cx: &mut App| {
            gpui_kit::init(cx);
            release_weave::ui::theme::install_theme(cx);

            let is_macos = cfg!(target_os = "macos");
            // Transparent titlebar with custom traffic lights is macOS-only; Windows and Linux retain standard decorations to prevent double titlebars.
            let titlebar = Some(TitlebarOptions {
                title: Some("ReleaseWeave".into()),
                appears_transparent: is_macos,
                traffic_light_position: if is_macos {
                    Some(Point {
                        x: px(12.0),
                        y: px(12.0),
                    })
                } else {
                    None
                },
            });

            let options = WindowOptions {
                titlebar,
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1024.0), px(720.0)),
                })),
                ..Default::default()
            };

            let db_path = paths::data_dir()
                .map(|d| d.join("release-weave.db"))
                .unwrap_or_else(|_| std::path::PathBuf::from("release-weave.db"));

            if let Err(e) = gpui_kit::open_window(options, cx, |_, cx| {
                let mut state = AppState::new(db_path);
                _ = state.load_projects_sync();
                let state_entity = cx.new(|_| state);
                cx.new(|cx| ShellView::new(state_entity, cx))
            }) {
                tracing::error!("Failed to open window: {e}");
            }
        });
}

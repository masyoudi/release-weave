use gpui_kit::App;
use gpui_kit::component::theme::{Theme, ThemeMode, ThemeRegistry};

pub const LIGHT_THEME_NAME: &str = "Carbon & Cobalt Light";
pub const DARK_THEME_NAME: &str = "Carbon & Cobalt Dark";

pub const CARBON_COBALT_THEME_JSON: &str = r##"{
  "name": "Carbon & Cobalt",
  "author": "ReleaseWeave",
  "themes": [
    {
      "name": "Carbon & Cobalt Light",
      "mode": "light",
      "radius": 6,
      "radius.lg": 10,
      "shadow": true,
      "colors": {
        "background": "#F9FAFB",
        "foreground": "#111827",
        "overlay": "#00000020",
        "group_box.background": "#FFFFFF",
        "group_box.foreground": "#111827",
        "sidebar.background": "#F3F4F6",
        "sidebar.foreground": "#111827",
        "sidebar.accent.background": "#E5E7EB",
        "sidebar.accent.foreground": "#1D4ED8",
        "sidebar.border": "#E5E7EB",
        "sidebar.primary.background": "#2563EB",
        "sidebar.primary.foreground": "#FFFFFF",
        "title_bar.background": "#F3F4F6",
        "title_bar.border": "#E5E7EB",
        "status_bar.background": "#F3F4F6",
        "status_bar.border": "#E5E7EB",
        "border": "#E5E7EB",
        "window.border": "#E5E7EB",
        "input.border": "#D1D5DB",
        "caret": "#111827",
        "ring": "#2563EB",
        "primary.background": "#2563EB",
        "primary.hover.background": "#1D4ED8",
        "primary.active.background": "#1E40AF",
        "primary.foreground": "#FFFFFF",
        "button.primary.background": "#2563EB",
        "button.primary.hover.background": "#1D4ED8",
        "button.primary.active.background": "#1E40AF",
        "button.primary.foreground": "#FFFFFF",
        "secondary.background": "#EEF2F6",
        "secondary.hover.background": "#E2E8F0",
        "secondary.active.background": "#CBD5E1",
        "secondary.foreground": "#111827",
        "button.secondary.background": "#EEF2F6",
        "button.secondary.hover.background": "#E2E8F0",
        "button.secondary.active.background": "#CBD5E1",
        "button.secondary.foreground": "#111827",
        "muted.background": "#F3F4F6",
        "muted.foreground": "#4B5563",
        "accent.background": "#DBEAFE",
        "accent.foreground": "#1E40AF",
        "popover.background": "#FFFFFF",
        "popover.foreground": "#111827",
        "list.background": "#FFFFFF",
        "list.even.background": "#F9FAFB",
        "list.head.background": "#F3F4F6",
        "list.hover.background": "#F3F4F6",
        "list.active.background": "#DBEAFE",
        "list.active.border": "#2563EB",
        "table.background": "#FFFFFF",
        "table.even.background": "#F9FAFB",
        "table.head.background": "#F3F4F6",
        "table.head.foreground": "#4B5563",
        "table.hover.background": "#F3F4F6",
        "table.active.background": "#DBEAFE",
        "table.active.border": "#2563EB",
        "table.row.border": "#E5E7EB",
        "tab.background": "#00000000",
        "tab.active.background": "#FFFFFF",
        "tab.active.foreground": "#111827",
        "tab.foreground": "#4B5563",
        "tab_bar.background": "#F3F4F6",
        "tab_bar.segmented.background": "#E5E7EB",
        "scrollbar.background": "#F3F4F600",
        "scrollbar.thumb.background": "#9CA3AFCC",
        "scrollbar.thumb.hover.background": "#6B7280",
        "selection.background": "#BFDBFE",
        "skeleton.background": "#E5E7EB",
        "switch.background": "#D1D5DB",
        "progress.bar.background": "#2563EB",
        "slider.background": "#2563EB",
        "slider.thumb.background": "#FFFFFF",
        "success.background": "#15803D",
        "success.hover.background": "#166534",
        "success.active.background": "#14532D",
        "success.foreground": "#FFFFFF",
        "button.success.background": "#15803D",
        "button.success.hover.background": "#166534",
        "button.success.active.background": "#14532D",
        "button.success.foreground": "#FFFFFF",
        "warning.background": "#B45309",
        "warning.hover.background": "#92400E",
        "warning.active.background": "#78350F",
        "warning.foreground": "#FFFFFF",
        "danger.background": "#B91C1C",
        "danger.hover.background": "#991B1B",
        "danger.active.background": "#7F1D1D",
        "danger.foreground": "#FFFFFF",
        "button.danger.background": "#B91C1C",
        "button.danger.hover.background": "#991B1B",
        "button.danger.active.background": "#7F1D1D",
        "button.danger.foreground": "#FFFFFF",
        "info.background": "#0369A1",
        "info.hover.background": "#075985",
        "info.active.background": "#0C4A6E",
        "info.foreground": "#FFFFFF"
      }
    },
    {
      "name": "Carbon & Cobalt Dark",
      "mode": "dark",
      "radius": 6,
      "radius.lg": 10,
      "shadow": true,
      "colors": {
        "background": "#111317",
        "foreground": "#F3F4F6",
        "overlay": "#00000040",
        "group_box.background": "#181B20",
        "group_box.foreground": "#F3F4F6",
        "sidebar.background": "#0B0C0E",
        "sidebar.foreground": "#F3F4F6",
        "sidebar.accent.background": "#1A1E26",
        "sidebar.accent.foreground": "#60A5FA",
        "sidebar.border": "#1E2128",
        "sidebar.primary.background": "#2563EB",
        "sidebar.primary.foreground": "#FFFFFF",
        "title_bar.background": "#0B0C0E",
        "title_bar.border": "#1E2128",
        "status_bar.background": "#0B0C0E",
        "status_bar.border": "#1E2128",
        "border": "#262930",
        "window.border": "#1E2128",
        "input.border": "#323640",
        "caret": "#F3F4F6",
        "ring": "#60A5FA",
        "primary.background": "#60A5FA",
        "primary.hover.background": "#93C5FD",
        "primary.active.background": "#3B82F6",
        "primary.foreground": "#0B0C0E",
        "button.primary.background": "#2563EB",
        "button.primary.hover.background": "#1D4ED8",
        "button.primary.active.background": "#1E40AF",
        "button.primary.foreground": "#FFFFFF",
        "secondary.background": "#22252C",
        "secondary.hover.background": "#2C3039",
        "secondary.active.background": "#181B20",
        "secondary.foreground": "#F3F4F6",
        "button.secondary.background": "#22252C",
        "button.secondary.hover.background": "#2C3039",
        "button.secondary.active.background": "#181B20",
        "button.secondary.foreground": "#F3F4F6",
        "muted.background": "#181B20",
        "muted.foreground": "#9CA3AF",
        "accent.background": "#1E3A8A",
        "accent.foreground": "#DBEAFE",
        "popover.background": "#181B20",
        "popover.foreground": "#F3F4F6",
        "list.background": "#181B20",
        "list.even.background": "#11131766",
        "list.head.background": "#11131799",
        "list.hover.background": "#22252C",
        "list.active.background": "#1E3A8A",
        "list.active.border": "#60A5FA",
        "table.background": "#181B20",
        "table.even.background": "#11131766",
        "table.head.background": "#111317",
        "table.head.foreground": "#9CA3AF",
        "table.hover.background": "#22252C",
        "table.active.background": "#1E3A8A",
        "table.active.border": "#60A5FA",
        "table.row.border": "#262930",
        "tab.background": "#00000000",
        "tab.active.background": "#181B20",
        "tab.active.foreground": "#F3F4F6",
        "tab.foreground": "#9CA3AF",
        "tab_bar.background": "#0B0C0E",
        "tab_bar.segmented.background": "#0B0C0E",
        "scrollbar.background": "#0B0C0E00",
        "scrollbar.thumb.background": "#3F4451CC",
        "scrollbar.thumb.hover.background": "#525866",
        "selection.background": "#1D4ED8",
        "skeleton.background": "#22252C",
        "switch.background": "#323640",
        "progress.bar.background": "#60A5FA",
        "slider.background": "#60A5FA",
        "slider.thumb.background": "#0B0C0E",
        "success.background": "#34D399",
        "success.hover.background": "#6EE7B7",
        "success.active.background": "#10B981",
        "success.foreground": "#064E3B",
        "button.success.background": "#15803D",
        "button.success.hover.background": "#166534",
        "button.success.active.background": "#14532D",
        "button.success.foreground": "#FFFFFF",
        "warning.background": "#FBBF24",
        "warning.hover.background": "#FCD34D",
        "warning.active.background": "#F59E0B",
        "warning.foreground": "#451A03",
        "danger.background": "#F87171",
        "danger.hover.background": "#FCA5A5",
        "danger.active.background": "#EF4444",
        "danger.foreground": "#450A0A",
        "button.danger.background": "#B91C1C",
        "button.danger.hover.background": "#991B1B",
        "button.danger.active.background": "#7F1D1D",
        "button.danger.foreground": "#FFFFFF",
        "info.background": "#38BDF8",
        "info.hover.background": "#7DD3FC",
        "info.active.background": "#0EA5E9",
        "info.foreground": "#082F49"
      }
    }
  ]
}"##;

pub fn install_theme(cx: &mut App) {
    if let Err(err) = ThemeRegistry::global_mut(cx).load_themes_from_str(CARBON_COBALT_THEME_JSON) {
        tracing::error!("Failed to load Carbon & Cobalt theme: {err}");
        return;
    }

    let light = ThemeRegistry::global(cx)
        .themes()
        .get(LIGHT_THEME_NAME)
        .cloned();
    let dark = ThemeRegistry::global(cx)
        .themes()
        .get(DARK_THEME_NAME)
        .cloned();

    if let Some(light_theme) = light {
        Theme::global_mut(cx).light_theme = light_theme;
    }
    if let Some(dark_theme) = dark {
        Theme::global_mut(cx).dark_theme = dark_theme;
    }

    Theme::change(ThemeMode::Dark, None, cx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::component::theme::ThemeSet;
    use std::rc::Rc;

    #[test]
    fn carbon_cobalt_theme_parses_and_maintains_surface_hierarchy() {
        let theme_set: ThemeSet =
            serde_json::from_str(CARBON_COBALT_THEME_JSON).expect("valid ThemeSet JSON");
        assert_eq!(theme_set.themes.len(), 2);

        let light_cfg = theme_set
            .themes
            .iter()
            .find(|t| t.name == LIGHT_THEME_NAME)
            .expect("light theme present")
            .clone();
        let dark_cfg = theme_set
            .themes
            .iter()
            .find(|t| t.name == DARK_THEME_NAME)
            .expect("dark theme present")
            .clone();

        let mut light_theme = Theme::default();
        light_theme.apply_config(&Rc::new(light_cfg));
        assert_ne!(light_theme.colors.sidebar, light_theme.colors.background);
        assert_ne!(light_theme.colors.group_box, light_theme.colors.background);
        assert_ne!(light_theme.colors.sidebar, light_theme.colors.group_box);

        let mut dark_theme = Theme::default();
        dark_theme.apply_config(&Rc::new(dark_cfg));
        assert_ne!(dark_theme.colors.sidebar, dark_theme.colors.background);
        assert_ne!(dark_theme.colors.group_box, dark_theme.colors.background);
        assert_ne!(dark_theme.colors.sidebar, dark_theme.colors.group_box);
    }
}

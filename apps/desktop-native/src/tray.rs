//! The Windows system tray: the WA Desk icon with an "Open WA Desk" / "Quit"
//! menu. Linux has no tray yet (it would pull GTK into the build); the module
//! compiles to a no-op there so the rest of the window is unchanged.

/// What the user asked for from the tray menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum TrayAction {
    Open,
    Quit,
}

/// Map a menu id back to an action (kept pure so it can be unit tested).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn action_for(id: &str) -> Option<TrayAction> {
    match id {
        "open" => Some(TrayAction::Open),
        "quit" => Some(TrayAction::Quit),
        _ => None,
    }
}

#[cfg(windows)]
pub struct Tray {
    _icon: tray_icon::TrayIcon,
}

#[cfg(windows)]
impl Tray {
    /// Install the tray icon. Returns `None` when the shell refuses it (a
    /// headless session, for example) - the app keeps working without one.
    pub fn new(icon: &egui::IconData) -> Option<Self> {
        use tray_icon::TrayIconBuilder;
        use tray_icon::menu::{Menu, MenuItem};

        let menu = Menu::new();
        let open = MenuItem::with_id("open", "Open WA Desk", true, None);
        let quit = MenuItem::with_id("quit", "Quit WA Desk", true, None);
        menu.append_items(&[&open, &quit]).ok()?;

        let image = tray_icon::Icon::from_rgba(icon.rgba.clone(), icon.width, icon.height).ok()?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("WA Desk")
            .with_icon(image)
            .build()
            .ok()?;
        // The menu items must outlive the menu handle for their ids to stay
        // valid; the tray owns the menu, and the ids are static strings.
        std::mem::forget((open, quit));
        crate::app::diag("tray: icon installed");
        Some(Self { _icon: tray })
    }

    /// The action the user picked since the last frame, if any.
    pub fn poll(&self) -> Option<TrayAction> {
        use tray_icon::TrayIconEvent;
        use tray_icon::menu::MenuEvent;

        if let Ok(event) = MenuEvent::receiver().try_recv() {
            return action_for(event.id.0.as_str());
        }
        if let Ok(event) = TrayIconEvent::receiver().try_recv()
            && matches!(
                event,
                TrayIconEvent::DoubleClick { .. } | TrayIconEvent::Click { .. }
            )
        {
            return Some(TrayAction::Open);
        }
        None
    }
}

#[cfg(not(windows))]
pub struct Tray;

#[cfg(not(windows))]
impl Tray {
    pub fn new(_icon: &egui::IconData) -> Option<Self> {
        None
    }

    pub fn poll(&self) -> Option<TrayAction> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_actions_are_distinct() {
        assert_ne!(TrayAction::Open, TrayAction::Quit);
    }

    #[test]
    fn menu_ids_map_to_actions() {
        assert_eq!(action_for("open"), Some(TrayAction::Open));
        assert_eq!(action_for("quit"), Some(TrayAction::Quit));
        assert_eq!(action_for("something-else"), None);
    }
}

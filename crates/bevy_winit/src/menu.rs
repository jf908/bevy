use muda::{Menu, PredefinedMenuItem, Submenu};

pub struct AppMenu {
    pub menu_bar: Menu,
    // pub edit_menu: Submenu,
}

impl AppMenu {
    pub fn new(menu_bar: Menu) -> Self {
        let app_menu = Submenu::new("App", true);
        app_menu
            .append_items(&[
                &PredefinedMenuItem::about(None, None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::services(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::quit(None),
            ])
            .ok();
        menu_bar.append(&app_menu).ok();

        let edit_menu = Submenu::new("&Edit", true);

        menu_bar.append_items(&[&edit_menu]).ok();

        let undo = PredefinedMenuItem::undo(None);
        let redo = PredefinedMenuItem::redo(None);

        let copy = PredefinedMenuItem::copy(None);
        let cut = PredefinedMenuItem::cut(None);
        let paste = PredefinedMenuItem::paste(None);
        let select_all = PredefinedMenuItem::select_all(None);

        edit_menu
            .append_items(&[
                &undo,
                &redo,
                &PredefinedMenuItem::separator(),
                &cut,
                &copy,
                &paste,
                &select_all,
            ])
            .ok();

        Self { menu_bar }
    }
}

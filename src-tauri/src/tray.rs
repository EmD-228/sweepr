//! Icon in the macOS menu bar and the Windows notification area.

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

/// Black on transparent, drawn from `design/icon/tray.svg`.
const ICON: &[u8] = include_bytes!("../icons/tray.png");

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Ouvrir Sweepr", true, None::<&str>)?;
    let scan = MenuItem::with_id(app, "scan", "Analyser maintenant", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quitter Sweepr", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &scan, &separator, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(icon()?)
        // macOS recolours a template image for the light and dark menu bar.
        .icon_as_template(true)
        .tooltip("Sweepr")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_window(app),
            "scan" => {
                show_window(app);
                let _ = app.emit("tray:scan", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

/// The black icon for macOS; white elsewhere, where the taskbar is dark by default
/// and template images do not exist.
fn icon() -> tauri::Result<Image<'static>> {
    let image = Image::from_bytes(ICON)?;
    if cfg!(target_os = "macos") {
        return Ok(image.to_owned());
    }
    let mut rgba = image.rgba().to_vec();
    for [r, g, b, _alpha] in rgba.as_chunks_mut::<4>().0 {
        (*r, *g, *b) = (255, 255, 255);
    }
    Ok(Image::new_owned(rgba, image.width(), image.height()))
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

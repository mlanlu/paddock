// The bundled binary must not open a console window on Windows. paddock does
// not target Windows, but the attribute is free and keeps `cargo` quiet.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    paddock_app_lib::run()
}

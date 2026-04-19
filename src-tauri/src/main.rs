#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    devil_connection_patcher_lib::run()
}

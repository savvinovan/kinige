// In a release build on Windows, do not open a console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    kinige_lib::run();
}

// В релизной сборке под Windows не открывать консоль.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    kinige_lib::run();
}

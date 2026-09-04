#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    iec104_simulator_slave_lib::run()
}

use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::game::GameState;

pub fn spawn_ipc(ipc_mowing_flag: Arc<AtomicU32>, ipc_game_state: Arc<Mutex<GameState>>) {
    std::thread::spawn(move || {
        let socket_path = "/tmp/grazz_ipc.sock";
        let _ = std::fs::remove_file(socket_path);

        let listener = UnixListener::bind(socket_path).expect("Failed to bind socket");

        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let mut buf = [0; 64];

                if let Ok(bytes_read) = stream.read(&mut buf) {
                    let request = String::from_utf8_lossy(&buf[..bytes_read])
                        .trim()
                        .to_uppercase();

                    match request.as_str() {
                        "MOW" => {
                            ipc_mowing_flag.fetch_add(1, Ordering::Relaxed);
                            let _ = stream.write_all(b"MOWER_DISPATCHED\n");
                        }
                        "BALANCE" => {
                            if let Ok(gs) = ipc_game_state.lock() {
                                let response = format!("${:.2}\n", gs.money);
                                let _ = stream.write_all(response.as_bytes());
                            }
                        }
                        "UP_FERT" => {
                            if let Ok(mut gs) = ipc_game_state.lock() {
                                let price = 1000.0 + (gs.fertilizer_level * 200) as f32;
                                if gs.money >= price {
                                    gs.money -= price;

                                    gs.fertilizer_level += 1;

                                    let _ = stream.write_all(b"FERT_UP\n");
                                }
                            }
                        }
                        "UP_MONEY" => {
                            if let Ok(mut gs) = ipc_game_state.lock() {
                                let price = 1000.0 + (gs.money_level * 200) as f32;
                                if gs.money >= price {
                                    gs.money -= price;

                                    gs.money_level += 1;

                                    let _ = stream.write_all(b"UP_MONEY\n");
                                }
                            }
                        }
                        "UP_MOWER" => {
                            if let Ok(mut gs) = ipc_game_state.lock() {
                                let price = 1000.0 + (gs.mower_level * 200) as f32;
                                if gs.money >= price {
                                    gs.money -= price;

                                    gs.mower_level += 1;

                                    let _ = stream.write_all(b"UP_MOWER\n");
                                }
                            }
                        }
                        "STATE" => {
                            if let Ok(gs) = ipc_game_state.lock() {
                                let _ = stream.write_all(gs.get_state().as_bytes());
                            }
                        }
                        _ => {
                            let _ = stream.write_all(b"UNKNOWN_COMMAND\n");
                        }
                    }
                }
            }
        }
    });
}

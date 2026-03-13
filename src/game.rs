use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GameState {
    pub total_grass_cut: f32,
    pub money: f32,
    pub mower_level: u32,
    pub fertilizer_level: u32,
    pub money_level: u32,
}

impl GameState {
    fn get_save_path() -> std::path::PathBuf {
        let proj_dirs = directories::ProjectDirs::from("com", "github", "grazz").unwrap();
        let config_dir = proj_dirs.config_dir();
        std::fs::create_dir_all(config_dir).unwrap();
        config_dir.join("save.json")
    }

    pub fn load() -> Self {
        let path = Self::get_save_path();
        if let Ok(data) = std::fs::read_to_string(path) {
            if let Ok(state) = serde_json::from_str(&data) {
                return state;
            }
        }

        Self {
            total_grass_cut: 0.0,
            money: 0.0,
            money_level: 1,
            mower_level: 1,
            fertilizer_level: 1,
        }
    }

    pub fn save(&self) {
        let path = Self::get_save_path();
        let data = serde_json::to_string_pretty(self).unwrap();
        let _ = std::fs::write(path, data);
    }

    pub fn get_state(&self) -> String {
        return serde_json::to_string_pretty(self).unwrap();
    }
}

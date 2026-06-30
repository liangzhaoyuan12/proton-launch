use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameConfig {
    pub name: String,
    pub executable: String,
    pub args: String,
    pub work_dir: String,
    pub renderer: String,
    pub env_vars: HashMap<String, String>,
}

impl GameConfig {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            executable: String::new(),
            args: String::new(),
            work_dir: String::new(),
            renderer: String::new(),
            env_vars: HashMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct ConfigFile {
    games: Vec<GameConfig>,
}

pub struct ConfigStore {
    path: PathBuf,
    games: Vec<GameConfig>,
}

impl ConfigStore {
    pub fn new() -> Self {
        let proj_dirs = ProjectDirs::from("com", "proton-launch", "proton-launch")
            .expect("could not determine project directories");
        let config_dir = proj_dirs.config_dir().to_path_buf();
        let path = config_dir.join("games.json");

        let games = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_json::from_str::<ConfigFile>(&content) {
                        Ok(cf) => cf.games,
                        Err(e) => {
                            eprintln!("failed to parse config: {e}");
                            Vec::new()
                        }
                    }
                }
                Err(e) => {
                    eprintln!("failed to read config: {e}");
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        ConfigStore { path, games }
    }

    pub fn games(&self) -> &[GameConfig] {
        &self.games
    }

    #[allow(dead_code)]
    pub fn games_mut(&mut self) -> &mut Vec<GameConfig> {
        &mut self.games
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let cf = ConfigFile { games: self.games.clone() };
        let json = serde_json::to_string_pretty(&cf).map_err(|e| e.to_string())?;
        fs::write(&self.path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn add_game(&mut self, game: GameConfig) {
        self.games.push(game);
    }

    pub fn remove_game(&mut self, idx: usize) {
        if idx < self.games.len() {
            self.games.remove(idx);
        }
    }

    pub fn update_game(&mut self, idx: usize, game: GameConfig) {
        if idx < self.games.len() {
            self.games[idx] = game;
        }
    }

    pub fn data_dir() -> PathBuf {
        let proj_dirs = ProjectDirs::from("com", "proton-launch", "proton-launch")
            .expect("could not determine project directories");
        let data_dir = proj_dirs.data_dir().to_path_buf();
        data_dir
    }
}

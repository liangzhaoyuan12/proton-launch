use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::i18n::Lang;

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

#[derive(Serialize, Deserialize)]
struct PrefsFile {
    lang: String,
}

impl Default for PrefsFile {
    fn default() -> Self {
        Self { lang: "zh".to_string() }
    }
}

pub struct ConfigStore {
    path: PathBuf,
    #[allow(dead_code)]
    prefs_path: PathBuf,
    games: Vec<GameConfig>,
}

impl ConfigStore {
    pub fn new() -> Self {
        let proj_dirs = ProjectDirs::from("com", "proton-launch", "proton-launch")
            .expect("could not determine project directories");
        let config_dir = proj_dirs.config_dir().to_path_buf();
        let path = config_dir.join("games.json");
        let prefs_path = config_dir.join("prefs.json");

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

        ConfigStore { path, prefs_path, games }
    }

    pub fn load_lang() -> Lang {
        let proj_dirs = ProjectDirs::from("com", "proton-launch", "proton-launch")
            .expect("could not determine project directories");
        let prefs_path = proj_dirs.config_dir().join("prefs.json");
        if prefs_path.exists() {
            if let Ok(content) = fs::read_to_string(&prefs_path) {
                if let Ok(prefs) = serde_json::from_str::<PrefsFile>(&content) {
                    return Lang::from_str(&prefs.lang);
                }
            }
        }
        Lang::Zh
    }

    pub fn save_lang(lang: Lang) {
        let proj_dirs = ProjectDirs::from("com", "proton-launch", "proton-launch")
            .expect("could not determine project directories");
        let config_dir = proj_dirs.config_dir();
        let prefs_path = config_dir.join("prefs.json");
        if let Ok(json) = serde_json::to_string(&PrefsFile { lang: lang.as_str().to_string() }) {
            let _ = fs::create_dir_all(config_dir);
            let _ = fs::write(&prefs_path, json);
        }
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

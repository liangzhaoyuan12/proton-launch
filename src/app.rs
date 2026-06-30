use std::collections::HashMap;
use std::thread;

use eframe::egui;
use egui::{CollapsingHeader, ScrollArea, SidePanel, TopBottomPanel};

use crate::config::{ConfigStore, GameConfig};
use crate::runner::Runner;

// ─── env var definitions ────────────────────────────────────────────────

/// 这些环境变量是 0/1 开关，直接显示为复选框，勾选=1
static BOOL_TOGGLE_KEYS: &[&str] = &[
    "UMU_USE_STEAM",
    "WINE_DO_NOT_CREATE_DXGI_DEVICE_MANAGER",
    "PROTON_NO_WM_DECORATION",
    "WINE_FULLSCREEN_FSR",
    "WINE_FULLSCREEN_INTEGER_SCALING",
    "PROTON_ENABLE_WAYLAND",
    "PROTON_ENABLE_HDR",
    "WINEESYNC",
    "WINEFSYNC",
    "NTSYNC",
    "WINE_LARGE_ADDRESS_AWARE",
    "WINE_HEAP_DELAY_FREE",
    "MESA_NO_ERROR",
    "__GL_THREADED_OPTIMIZATIONS",
    "__GL_SHADER_DISK_CACHE",
    "WINE_HIDE_NVIDIA_GPU",
    "__NV_PRIME_RENDER_OFFLOAD",
    "PROTON_DLSS_UPGRADE",
    "PROTON_DLSS_INDICATOR",
    "PROTON_FSR4_UPGRADE",
    "FSR4_UPGRADE",
    "PROTON_FSR4_RDNA3_UPGRADE",
    "PROTON_XESS_UPGRADE",
    "MANGOHUD",
    "PROTON_PREFER_SDL",
    "COPYPREFIX",
    "SteamDeck",
    "WINE_DISABLE_FAST_SYNC",
    "WINE_DISABLE_WRITE_WATCH",
    "STAGING_SHARED_MEMORY",
];

struct EnvVarDef {
    key: &'static str,
    label: &'static str,
}

struct EnvCategory {
    name: &'static str,
    vars: &'static [EnvVarDef],
}

static ENV_CATEGORIES: &[EnvCategory] = &[
    EnvCategory {
        name: "核心 (Core)",
        vars: &[
            EnvVarDef { key: "WINEPREFIX", label: "Wine Prefix 路径" },
            EnvVarDef { key: "GAMEID", label: "游戏 ID" },
            EnvVarDef { key: "STORE", label: "商店 ID (如 steam, none)" },
            EnvVarDef { key: "UMU_USE_STEAM", label: "使用 Steam 运行时环境" },
        ],
    },
    EnvCategory {
        name: "DLL / Windows 环境",
        vars: &[
            EnvVarDef { key: "WINEDLLOVERRIDES", label: "DLL 覆盖 (如 d3d9=n,b)" },
            EnvVarDef { key: "WINEDLLPATH", label: "额外 DLL 搜索路径" },
            EnvVarDef { key: "WINEPATH", label: "Windows PATH" },
            EnvVarDef { key: "STEAM_COMPAT_DATA_PATH", label: "Proton 兼容数据路径" },
        ],
    },
    EnvCategory {
        name: "图形渲染 (Graphics)",
        vars: &[
            EnvVarDef { key: "WINE_D3D_CONFIG", label: "D3D 配置 (renderer=vulkan 等)" },
            EnvVarDef { key: "WINE_DO_NOT_CREATE_DXGI_DEVICE_MANAGER", label: "修复过场动画色块 (1=启用)" },
            EnvVarDef { key: "PROTON_NO_WM_DECORATION", label: "禁用窗口装饰 (1=启用)" },
            EnvVarDef { key: "DXVK_HUD", label: "DXVK HUD (fps, full 等)" },
            EnvVarDef { key: "DXVK_CONFIG_FILE", label: "DXVK 配置文件路径" },
            EnvVarDef { key: "DXVK_STATE_CACHE_PATH", label: "DXVK 着色器缓存路径" },
            EnvVarDef { key: "VKD3D_CONFIG", label: "VKD3D 配置 (dxr, dxr11 等)" },
            EnvVarDef { key: "VKD3D_SHADER_CACHE_PATH", label: "VKD3D 着色器缓存路径" },
            EnvVarDef { key: "WINE_FULLSCREEN_FSR", label: "AMD FSR 1 (0=禁用, 1=启用)" },
            EnvVarDef { key: "WINE_FULLSCREEN_FSR_MODE", label: "FSR 质量模式 (0-4)" },
            EnvVarDef { key: "WINE_FULLSCREEN_FSR_STRENGTH", label: "FSR 锐化强度 (0-5)" },
            EnvVarDef { key: "WINE_FULLSCREEN_FSR_CUSTOM_MODE", label: "FSR 虚拟分辨率 (如 1920x1080)" },
            EnvVarDef { key: "WINE_FULLSCREEN_INTEGER_SCALING", label: "整数缩放 (1=启用)" },
        ],
    },
    EnvCategory {
        name: "Wayland",
        vars: &[
            EnvVarDef { key: "PROTON_ENABLE_WAYLAND", label: "启用 Wayland (0/1)" },
            EnvVarDef { key: "PROTON_ENABLE_HDR", label: "启用 HDR (0/1)" },
        ],
    },
    EnvCategory {
        name: "同步机制 (Sync)",
        vars: &[
            EnvVarDef { key: "WINEESYNC", label: "esync (0/1)" },
            EnvVarDef { key: "WINEFSYNC", label: "fsync (0/1)" },
            EnvVarDef { key: "NTSYNC", label: "ntsync (0/1)" },
        ],
    },
    EnvCategory {
        name: "内存和进程 (Memory/CPU)",
        vars: &[
            EnvVarDef { key: "WINE_LARGE_ADDRESS_AWARE", label: "32位大内存支持 (1=启用)" },
            EnvVarDef { key: "WINE_HEAP_DELAY_FREE", label: "延迟堆释放 (1=启用)" },
            EnvVarDef { key: "WINE_CPU_TOPOLOGY", label: "CPU 拓扑 (如 4:8)" },
        ],
    },
    EnvCategory {
        name: "音频 (Audio)",
        vars: &[
            EnvVarDef { key: "PULSE_LATENCY_MSEC", label: "PulseAudio 延迟 (ms)" },
            EnvVarDef { key: "SDL_AUDIODRIVER", label: "SDL 音频后端 (pulse/alsa/pipewire)" },
            EnvVarDef { key: "PIPEWIRE_LATENCY", label: "PipeWire 延迟 (如 256/48000)" },
        ],
    },
    EnvCategory {
        name: "Vulkan",
        vars: &[
            EnvVarDef { key: "VK_ICD_FILENAMES", label: "Vulkan ICD 文件路径" },
            EnvVarDef { key: "AMD_VULKAN_ICD", label: "AMD 驱动选择 (RADV/AMDVLK)" },
        ],
    },
    EnvCategory {
        name: "Mesa / OpenGL",
        vars: &[
            EnvVarDef { key: "MESA_GL_VERSION_OVERRIDE", label: "OpenGL 版本 (如 4.6)" },
            EnvVarDef { key: "MESA_GLSL_VERSION_OVERRIDE", label: "GLSL 版本 (如 460)" },
            EnvVarDef { key: "MESA_NO_ERROR", label: "禁用 GL 错误检查 (0/1)" },
            EnvVarDef { key: "mesa_glthread", label: "Mesa 多线程 (true/false)" },
            EnvVarDef { key: "MESA_LOADER_DRIVER_OVERRIDE", label: "Mesa 驱动 (radeonsi/iris/zink)" },
        ],
    },
    EnvCategory {
        name: "NVIDIA",
        vars: &[
            EnvVarDef { key: "__GL_THREADED_OPTIMIZATIONS", label: "OpenGL 多线程 (0/1)" },
            EnvVarDef { key: "__GL_SHADER_DISK_CACHE", label: "着色器缓存 (0/1)" },
            EnvVarDef { key: "__GL_SHADER_DISK_CACHE_PATH", label: "着色器缓存路径" },
            EnvVarDef { key: "WINE_HIDE_NVIDIA_GPU", label: "隐藏 NVIDIA GPU (1=隐藏)" },
            EnvVarDef { key: "__NV_PRIME_RENDER_OFFLOAD", label: "Prime 渲染卸载 (1=启用)" },
            EnvVarDef { key: "__GLX_VENDOR_LIBRARY_NAME", label: "GLX 提供方 (nvidia/mesa)" },
        ],
    },
    EnvCategory {
        name: "DLSS / FSR / XeSS",
        vars: &[
            EnvVarDef { key: "PROTON_DLSS_UPGRADE", label: "DLSS 自动更新 (0/1)" },
            EnvVarDef { key: "PROTON_DLSS_INDICATOR", label: "DLSS 指示器 (0/1)" },
            EnvVarDef { key: "PROTON_FSR4_UPGRADE", label: "FSR4 自动更新 - Proton-CachyOS (0/1)" },
            EnvVarDef { key: "FSR4_UPGRADE", label: "FSR4 自动更新 - GE-Proton (0/1)" },
            EnvVarDef { key: "PROTON_FSR4_RDNA3_UPGRADE", label: "RDNA3 FSR4 (0/1)" },
            EnvVarDef { key: "PROTON_XESS_UPGRADE", label: "XeSS 自动更新 (0/1)" },
        ],
    },
    EnvCategory {
        name: "性能监控 (Performance)",
        vars: &[
            EnvVarDef { key: "GALLIUM_HUD", label: "Gallium3D HUD (fps,cpu 等)" },
            EnvVarDef { key: "MANGOHUD", label: "MangoHud (0=禁用, 1=启用)" },
            EnvVarDef { key: "MANGOHUD_CONFIG", label: "MangoHud 配置" },
            EnvVarDef { key: "MANGOHUD_CONFIGFILE", label: "MangoHud 配置文件路径" },
        ],
    },
    EnvCategory {
        name: "输入设备 (Input)",
        vars: &[
            EnvVarDef { key: "PROTON_PREFER_SDL", label: "优先使用 SDL 输入 (0/1)" },
            EnvVarDef { key: "SDL_GAMECONTROLLERCONFIG", label: "手柄映射配置" },
            EnvVarDef { key: "SDL_JOYSTICK_DEVICE", label: "输入设备 (如 js0)" },
        ],
    },
    EnvCategory {
        name: "字体 (Font)",
        vars: &[
            EnvVarDef { key: "FREETYPE_PROPERTIES", label: "FreeType 渲染配置" },
        ],
    },
    EnvCategory {
        name: "其他 (Other)",
        vars: &[
            EnvVarDef { key: "COPYPREFIX", label: "复制前缀 (0/1)" },
            EnvVarDef { key: "SteamDeck", label: "SteamDeck 模式 (0/1)" },
            EnvVarDef { key: "PROTON_ADD_CONFIG", label: "快捷参数 (如 fsr4,wayland,hdr)" },
            EnvVarDef { key: "WINE_DISABLE_FAST_SYNC", label: "禁用快速同步 (0/1)" },
            EnvVarDef { key: "WINE_DISABLE_WRITE_WATCH", label: "禁用写入检测 (0/1)" },
            EnvVarDef { key: "STAGING_SHARED_MEMORY", label: "共享内存 (0/1)" },
            EnvVarDef { key: "WINEVERPATH", label: "Wine 版本路径" },
            EnvVarDef { key: "WINE", label: "Wine 可执行文件路径" },
        ],
    },
];

// ─── command output polling ─────────────────────────────────────────────

struct RunningState {
    child: std::process::Child,
    #[allow(dead_code)]
    stdout_thread: thread::JoinHandle<Vec<u8>>,
    #[allow(dead_code)]
    stderr_thread: thread::JoinHandle<Vec<u8>>,
    done: bool,
}

// ─── main app ───────────────────────────────────────────────────────────

pub struct ProtonApp {
    // data
    store: ConfigStore,
    runner: Runner,

    // selection / editing
    selected_idx: Option<usize>,

    // custom env var entry fields
    custom_key: String,
    custom_value: String,

    // status / messages
    status: String,
    status_is_err: bool,

    // running processes (index → state)
    running: HashMap<usize, RunningState>,
}

impl Default for ProtonApp {
    fn default() -> Self {
        let store = ConfigStore::new();
        let runner = Runner::new();

        let mut app = ProtonApp {
            store,
            runner,
            selected_idx: None,
            custom_key: String::new(),
            custom_value: String::new(),
            status: "就绪".to_string(),
            status_is_err: false,
            running: HashMap::new(),
        };

        // extract umu-run early
        if let Err(e) = app.runner.extract_umu() {
            eprintln!("warning: failed to extract umu-run: {e}");
        }

        // auto-select first game
        if !app.store.games().is_empty() {
            app.selected_idx = Some(0);
        }

        app
    }
}

impl eframe::App for ProtonApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── check running processes ──────────────────────────────────────
        self.poll_running();

        // ── top bar ──────────────────────────────────────────────────────
        TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Proton Launch Manager");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.colored_label(
                        egui::Color32::GRAY,
                        &format!("v{}", env!("CARGO_PKG_VERSION")),
                    );
                });
            });
        });

        // ── status bar ───────────────────────────────────────────────────
        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            let color = if self.status_is_err {
                egui::Color32::RED
            } else {
                egui::Color32::GREEN
            };
            ui.colored_label(color, &self.status);
            if !self.running.is_empty() {
                ui.label(format!("运行中: {}", self.running.len()));
            }
        });

        // ── side panel: game list ────────────────────────────────────────
        SidePanel::left("game_list")
            .resizable(true)
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.strong("游戏列表");

                    ui.horizontal(|ui| {
                        if ui.button("＋ 添加").clicked() {
                            self.add_game_dialog();
                        }
                        if ui.button("✕ 删除").clicked() {
                            self.delete_selected();
                        }
                    });

                    ui.separator();

                    let games = self.store.games().to_vec();
                    let mut to_select = self.selected_idx;

                    ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            for (i, game) in games.iter().enumerate() {
                                let selected = self.selected_idx == Some(i);
                                let running = self.running.contains_key(&i);
                                let label = if game.name.is_empty() {
                                    format!("未命名 #{}", i + 1)
                                } else {
                                    game.name.clone()
                                };
                                let display = if running {
                                    format!("▶ {}", label)
                                } else {
                                    label.clone()
                                };
                                if ui
                                    .selectable_label(selected, &display)
                                    .clicked()
                                {
                                    to_select = Some(i);
                                    self.update_status(&format!("已选择: {}", label), false);
                                }
                            }
                        });

                    self.selected_idx = to_select;

                    // load on selection change
                    if self.selected_idx.is_some() && self.selected_idx.unwrap() >= self.store.games().len() {
                        self.selected_idx = None;
                    }
                });
            });

        // ── central panel: game config ───────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(idx) = self.selected_idx {
                let games = self.store.games().to_vec();
                if idx < games.len() {
                    let mut game = games[idx].clone();

                    // top action bar
                    let running_here = self.running.get(&idx).and_then(|r| {
                        if !r.done { Some(r.child.id()) } else { None }
                    });

                    ui.horizontal(|ui| {
                        if running_here.is_some() {
                            ui.colored_label(egui::Color32::YELLOW, "▶ 运行中");
                        }

                        if ui
                            .button("💾 保存配置")
                            .clicked()
                        {
                            self.store.update_game(idx, game.clone());
                            match self.store.save() {
                                Ok(_) => self.update_status("配置已保存", false),
                                Err(e) => self.update_status(&format!("保存失败: {e}"), true),
                            }
                        }

                        if ui
                            .button("▶ 运行")
                            .clicked()
                        {
                            self.store.update_game(idx, game.clone());
                            self.launch_game(idx);
                        }

                        if let Some(pid) = running_here {
                            ui.separator();
                            ui.label(format!("PID: {pid}"));
                            if ui.button("⏹ 停止").clicked() {
                                self.kill_running(idx);
                            }
                        }
                    });

                    ui.separator();

                    ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            self.show_game_config(ui, &mut game);

                            ui.separator();

                            self.show_env_vars(ui, &mut game);

                            ui.separator();

                            self.show_custom_env_vars(ui, &mut game);
                        });

                    // auto-save to memory on every frame
                    self.store.update_game(idx, game);
                } else {
                    ui.label("请选择一个游戏进行配置");
                }
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.heading("Proton Launch Manager");
                    ui.label("欢迎使用！");
                    ui.label("点击左侧「＋ 添加」创建一个新游戏配置，");
                    ui.label("或选择一个已有游戏进行编辑。");
                });
            }
        });

        // request repaint while any process is running
        if !self.running.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}

impl ProtonApp {
    // ─── UI: game config basic fields ────────────────────────────────────

    fn show_game_config(&mut self, ui: &mut egui::Ui, game: &mut GameConfig) -> bool {
        let mut changed = false;

        ui.strong("基本设置");

        ui.horizontal(|ui| {
            ui.label("名称:");
            changed |= ui.text_edit_singleline(&mut game.name).changed();
        });

        ui.horizontal(|ui| {
            ui.label("可执行文件:");
            changed |= ui.text_edit_singleline(&mut game.executable).changed();
            if ui.button("浏览...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("选择可执行文件")
                    .pick_file()
                {
                    game.executable = path.display().to_string();
                    changed = true;
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("命令行参数:");
            changed |= ui.text_edit_singleline(&mut game.args).changed();
        });

        ui.horizontal(|ui| {
            ui.label("工作目录:");
            changed |= ui.text_edit_singleline(&mut game.work_dir).changed();
            if ui.button("浏览...").clicked() {
                if let Some(dir) = rfd::FileDialog::new()
                    .set_title("选择工作目录")
                    .pick_folder()
                {
                    game.work_dir = dir.display().to_string();
                    changed = true;
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("Proton 版本:");
            let mut proton_val = game.env_vars.get("PROTONPATH").cloned().unwrap_or_default();
            changed |= ui.add(
                egui::TextEdit::singleline(&mut proton_val)
                    .hint_text("路径/版本号 (如 GE-Proton9-5)")
            ).changed();
            if ui.button("浏览...").clicked() {
                if let Some(dir) = rfd::FileDialog::new()
                    .set_title("选择 Proton 目录")
                    .pick_folder()
                {
                    proton_val = dir.display().to_string();
                    changed = true;
                }
            }
            if !proton_val.is_empty() {
                game.env_vars.insert("PROTONPATH".to_string(), proton_val);
            } else {
                game.env_vars.remove("PROTONPATH");
            }
        });

        ui.horizontal(|ui| {
            ui.label("修改器/注入器:");
            let mut debug_val = game.env_vars.get("PROTON_REMOTE_DEBUG_CMD").cloned().unwrap_or_default();
            changed |= ui.add(
                egui::TextEdit::singleline(&mut debug_val)
                    .hint_text("可执行文件路径")
            ).changed();
            if ui.button("浏览...").clicked() {
                if let Some(f) = rfd::FileDialog::new()
                    .set_title("选择修改器/注入器")
                    .pick_file()
                {
                    debug_val = f.display().to_string();
                    changed = true;
                }
            }
            if !debug_val.is_empty() {
                game.env_vars.insert("PROTON_REMOTE_DEBUG_CMD".to_string(), debug_val);
            } else {
                game.env_vars.remove("PROTON_REMOTE_DEBUG_CMD");
            }
        });

        ui.add_space(8.0);
        ui.strong("渲染器设置");
        ui.label("启动参数，非所有游戏都支持");
        let renderers = ["", "-dx11", "-dx12", "-opengl", "-vulkan"];
        let labels = ["不指定", "DirectX 11 (-dx11)", "DirectX 12 (-dx12)", "OpenGL (-opengl)", "Vulkan (-vulkan)"];
        let mut sel = renderers.iter().position(|r| *r == game.renderer).unwrap_or(0);
        egui::ComboBox::from_id_salt("renderer_selector")
            .selected_text(labels[sel])
            .show_ui(ui, |ui| {
                for (i, label) in labels.iter().enumerate() {
                    ui.selectable_value(&mut sel, i, *label);
                }
            });
        if game.renderer != renderers[sel] {
            game.renderer = renderers[sel].to_string();
            changed = true;
        }

        changed
    }

    // ─── UI: environment variables (categorized) ─────────────────────────

    fn show_env_vars(&mut self, ui: &mut egui::Ui, game: &mut GameConfig) {
        ui.strong("环境变量");
        ui.label("勾选并填写需要的环境变量，留空则不设置");
        ui.add_space(4.0);

        for category in ENV_CATEGORIES {
            let count = category
                .vars
                .iter()
                .filter(|v| game.env_vars.contains_key(v.key))
                .count();

            let header_label = format!(
                "{}  (已设置 {}/{})",
                category.name,
                count,
                category.vars.len()
            );

            CollapsingHeader::new(header_label)
                .default_open(count > 0)
                .show(ui, |ui| {
                    for var_def in category.vars {
                        self.show_single_env_var(ui, game, var_def.key, var_def.label);
                    }
                });
        }
    }

    fn show_single_env_var(
        &mut self,
        ui: &mut egui::Ui,
        game: &mut GameConfig,
        key: &str,
        label: &str,
    ) {
        let mut value = game.env_vars.get(key).cloned().unwrap_or_default();
        let mut enabled = game.env_vars.contains_key(key);
        let is_path = key == "PROTONPATH" || key == "WINEPREFIX" || key == "WINE";
        let is_toggle = BOOL_TOGGLE_KEYS.contains(&key);

        ui.horizontal(|ui| {
            if is_toggle {
                let mut checked = enabled;
                if ui.checkbox(&mut checked, label).clicked() {
                    enabled = checked;
                    if enabled {
                        value = "1".to_string();
                    }
                }
            } else {
                ui.checkbox(&mut enabled, "");
                ui.label(label);
                if enabled {
                    ui.add(
                        egui::TextEdit::singleline(&mut value)
                            .desired_width(f32::INFINITY)
                            .hint_text("值"),
                    );
                }
                if is_path {
                    let btn = if key == "WINE" {
                        ui.button("📂 浏览文件...")
                    } else {
                        ui.button("📂 浏览目录...")
                    };
                    if btn.clicked() {
                        let dialog = rfd::FileDialog::new();
                        let picked = if key == "WINE" {
                            dialog.set_title("选择 Proton/Wine 可执行文件").pick_file()
                        } else {
                            dialog.set_title("选择 Proton 目录").pick_folder()
                        };
                        if let Some(p) = picked {
                            value = p.display().to_string();
                            enabled = true;
                        }
                    }
                }
            }
        });

        if is_toggle && enabled {
            game.env_vars.insert(key.to_string(), "1".to_string());
        } else if enabled && !value.is_empty() {
            game.env_vars.insert(key.to_string(), value);
        } else if !enabled {
            game.env_vars.remove(key);
        } else {
            game.env_vars.insert(key.to_string(), value);
        }
    }

    // ─── UI: custom env vars ─────────────────────────────────────────────

    fn show_custom_env_vars(&mut self, ui: &mut egui::Ui, game: &mut GameConfig) -> bool {
        let mut changed = false;

        CollapsingHeader::new("自定义环境变量")
            .default_open(false)
            .show(ui, |ui| {
                // list existing custom vars
                let known_keys: std::collections::HashSet<&str> = ENV_CATEGORIES
                    .iter()
                    .flat_map(|c| c.vars.iter().map(|v| v.key))
                    .collect();

                let custom_keys: Vec<String> = game
                    .env_vars
                    .keys()
                    .filter(|k| !known_keys.contains(k.as_str()))
                    .cloned()
                    .collect();

                if custom_keys.is_empty() {
                    ui.colored_label(egui::Color32::GRAY, "暂无自定义变量");
                } else {
                    for key in &custom_keys {
                        let mut val = game.env_vars.get(key.as_str()).cloned().unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.monospace(key);
                            if ui.text_edit_singleline(&mut val).changed() {
                                game.env_vars.insert(key.clone(), val);
                                changed = true;
                            }
                            if ui.button("✕").clicked() {
                                game.env_vars.remove(key.as_str());
                                changed = true;
                            }
                        });
                    }
                }

                ui.separator();

                // add new custom var
                ui.horizontal(|ui| {
                    ui.label("新增:");
                    changed |= ui
                        .add(egui::TextEdit::singleline(&mut self.custom_key).hint_text("变量名"))
                        .changed();
                    changed |= ui
                        .add(egui::TextEdit::singleline(&mut self.custom_value).hint_text("值"))
                        .changed();
                    if ui.button("添加").clicked() && !self.custom_key.is_empty() {
                        game.env_vars
                            .insert(self.custom_key.clone(), self.custom_value.clone());
                        self.custom_key.clear();
                        self.custom_value.clear();
                        changed = true;
                    }
                });
            });

        changed
    }

    // ─── game management ─────────────────────────────────────────────────

    fn add_game_dialog(&mut self) {
        let count = self.store.games().len();
        let name = format!("新游戏 #{}", count + 1);
        let game = GameConfig::new(&name);
        self.store.add_game(game);
        let idx = self.store.games().len() - 1;
        self.selected_idx = Some(idx);
        self.update_status(&format!("已添加: {name}"), false);
        if let Err(e) = self.store.save() {
            self.update_status(&format!("保存失败: {e}"), true);
        }
    }

    fn delete_selected(&mut self) {
        if let Some(idx) = self.selected_idx {
            self.running.remove(&idx);
            let name = self.store.games()[idx].name.clone();
            self.store.remove_game(idx);
            self.selected_idx = if self.store.games().is_empty() {
                None
            } else {
                Some(if idx >= self.store.games().len() {
                    self.store.games().len() - 1
                } else {
                    idx
                })
            };
            self.update_status(&format!("已删除: {name}"), false);
            if let Err(e) = self.store.save() {
                self.update_status(&format!("保存失败: {e}"), true);
            }
        }
    }

    // ─── launch ──────────────────────────────────────────────────────────

    fn launch_game(&mut self, idx: usize) {
        let game = if idx < self.store.games().len() {
            self.store.games()[idx].clone()
        } else {
            return;
        };

        if game.executable.is_empty() {
            self.update_status("请先选择可执行文件", true);
            return;
        }

        match self.runner.run_game(&game) {
            Ok(mut child) => {
                let stdout = child.stdout.take();
                let stderr = child.stderr.take();

                let stdout_thread = thread::spawn(move || {
                    if let Some(mut out) = stdout {
                        use std::io::Read;
                        let mut buf = Vec::new();
                        let _ = out.read_to_end(&mut buf);
                        buf
                    } else {
                        Vec::new()
                    }
                });

                let stderr_thread = thread::spawn(move || {
                    if let Some(mut err) = stderr {
                        use std::io::Read;
                        let mut buf = Vec::new();
                        let _ = err.read_to_end(&mut buf);
                        buf
                    } else {
                        Vec::new()
                    }
                });

                let pid = child.id();
                self.running.insert(idx, RunningState {
                    child,
                    stdout_thread,
                    stderr_thread,
                    done: false,
                });

                self.update_status(
                    &format!("已启动: {} (PID {})", game.name, pid),
                    false,
                );
            }
            Err(e) => {
                self.update_status(&format!("启动失败: {e}"), true);
            }
        }
    }

    fn poll_running(&mut self) {
        let mut done_idxs: Vec<(usize, String, bool)> = Vec::new();

        for (&idx, running) in &mut self.running {
            if running.done {
                continue;
            }
            match running.child.try_wait() {
                Ok(Some(status)) => {
                    running.done = true;
                    let code = status.code().unwrap_or(-1);
                    let name = self.store.games().get(idx)
                        .map(|g| g.name.clone())
                        .unwrap_or_default();
                    let msg = if code == 0 {
                        format!("「{name}」已正常退出")
                    } else {
                        format!("「{name}」退出，退出码: {code}")
                    };
                    done_idxs.push((idx, msg, code != 0));
                }
                Ok(None) => {}
                Err(e) => {
                    running.done = true;
                    let name = self.store.games().get(idx)
                        .map(|g| g.name.clone())
                        .unwrap_or_default();
                    done_idxs.push((idx, format!("「{name}」进程错误: {e}"), true));
                }
            }
        }

        for (_, msg, is_err) in &done_idxs {
            self.update_status(msg, *is_err);
        }

        for (idx, _, _) in &done_idxs {
            self.running.remove(idx);
        }
    }

    fn kill_running(&mut self, idx: usize) {
        if let Some(mut running) = self.running.remove(&idx) {
            let pid = running.child.id() as i32;
            // SIGTERM first, then SIGKILL to process group
            unsafe {
                libc::kill(-pid, libc::SIGTERM);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
            let _ = running.child.wait();
            let name = self.store.games().get(idx)
                .map(|g| g.name.clone())
                .unwrap_or_default();
            self.update_status(&format!("「{name}」进程已终止"), false);
        }
    }

    // ─── status ──────────────────────────────────────────────────────────

    fn update_status(&mut self, msg: &str, is_err: bool) {
        self.status = msg.to_string();
        self.status_is_err = is_err;
    }
}

//! 环境变量清单：分类、键名、中文标签与控件类型判定。
//!
//! 纯数据定义，供展示层逐条渲染为 libadwaita 行控件。

/// 单个环境变量的定义。
pub struct EnvVarDef {
    pub key: &'static str,
    pub label: &'static str,
}

/// 一组环境变量（对应界面上的一个可折叠分类）。
pub struct EnvCategory {
    pub name: &'static str,
    pub vars: &'static [EnvVarDef],
}

/// 0/1 开关型环境变量：界面渲染为开关行，勾选即写入 "1"。
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

/// 需要「选择文件」按钮的变量。
static FILE_KEYS: &[&str] = &["WINE"];

/// 需要「选择目录」按钮的变量。
static DIR_KEYS: &[&str] = &["PROTONPATH", "WINEPREFIX", "STEAM_COMPAT_DATA_PATH"];

/// 不在分类清单里、但界面上有独立输入行的变量（改动时同步 `page::config`）。
static EXTRA_KNOWN_KEYS: &[&str] = &["PROTONPATH", "PROTON_REMOTE_DEBUG_CMD"];

/// 全部环境变量分类（界面顺序与本数组顺序一致）。
pub static ENV_CATEGORIES: &[EnvCategory] = &[
    EnvCategory {
        name: "核心",
        vars: &[
            EnvVarDef {
                key: "WINEPREFIX",
                label: "Wine Prefix 路径",
            },
            EnvVarDef {
                key: "GAMEID",
                label: "游戏 ID",
            },
            EnvVarDef {
                key: "STORE",
                label: "商店 ID（如 steam、none）",
            },
            EnvVarDef {
                key: "UMU_USE_STEAM",
                label: "使用 Steam 运行时环境",
            },
        ],
    },
    EnvCategory {
        name: "DLL / Windows 环境",
        vars: &[
            EnvVarDef {
                key: "WINEDLLOVERRIDES",
                label: "DLL 覆盖（如 d3d9=n,b）",
            },
            EnvVarDef {
                key: "WINEDLLPATH",
                label: "额外 DLL 搜索路径",
            },
            EnvVarDef {
                key: "WINEPATH",
                label: "Windows PATH",
            },
            EnvVarDef {
                key: "STEAM_COMPAT_DATA_PATH",
                label: "Proton 兼容数据路径",
            },
        ],
    },
    EnvCategory {
        name: "图形渲染",
        vars: &[
            EnvVarDef {
                key: "WINE_D3D_CONFIG",
                label: "D3D 配置（renderer=vulkan 等）",
            },
            EnvVarDef {
                key: "WINE_DO_NOT_CREATE_DXGI_DEVICE_MANAGER",
                label: "修复过场动画色块（1=启用）",
            },
            EnvVarDef {
                key: "PROTON_NO_WM_DECORATION",
                label: "禁用窗口装饰（1=启用）",
            },
            EnvVarDef {
                key: "DXVK_HUD",
                label: "DXVK HUD（fps、full 等）",
            },
            EnvVarDef {
                key: "DXVK_CONFIG_FILE",
                label: "DXVK 配置文件路径",
            },
            EnvVarDef {
                key: "DXVK_STATE_CACHE_PATH",
                label: "DXVK 着色器缓存路径",
            },
            EnvVarDef {
                key: "VKD3D_CONFIG",
                label: "VKD3D 配置（dxr、dxr11 等）",
            },
            EnvVarDef {
                key: "VKD3D_SHADER_CACHE_PATH",
                label: "VKD3D 着色器缓存路径",
            },
            EnvVarDef {
                key: "WINE_FULLSCREEN_FSR",
                label: "AMD FSR 1（0=禁用，1=启用）",
            },
            EnvVarDef {
                key: "WINE_FULLSCREEN_FSR_MODE",
                label: "FSR 质量模式（0-4）",
            },
            EnvVarDef {
                key: "WINE_FULLSCREEN_FSR_STRENGTH",
                label: "FSR 锐化强度（0-5）",
            },
            EnvVarDef {
                key: "WINE_FULLSCREEN_FSR_CUSTOM_MODE",
                label: "FSR 虚拟分辨率（如 1920x1080）",
            },
            EnvVarDef {
                key: "WINE_FULLSCREEN_INTEGER_SCALING",
                label: "整数缩放（1=启用）",
            },
        ],
    },
    EnvCategory {
        name: "Wayland",
        vars: &[
            EnvVarDef {
                key: "PROTON_ENABLE_WAYLAND",
                label: "启用 Wayland（0/1）",
            },
            EnvVarDef {
                key: "PROTON_ENABLE_HDR",
                label: "启用 HDR（0/1）",
            },
        ],
    },
    EnvCategory {
        name: "同步机制",
        vars: &[
            EnvVarDef {
                key: "WINEESYNC",
                label: "esync（0/1）",
            },
            EnvVarDef {
                key: "WINEFSYNC",
                label: "fsync（0/1）",
            },
            EnvVarDef {
                key: "NTSYNC",
                label: "ntsync（0/1）",
            },
        ],
    },
    EnvCategory {
        name: "内存和进程",
        vars: &[
            EnvVarDef {
                key: "WINE_LARGE_ADDRESS_AWARE",
                label: "32 位大内存支持（1=启用）",
            },
            EnvVarDef {
                key: "WINE_HEAP_DELAY_FREE",
                label: "延迟堆释放（1=启用）",
            },
            EnvVarDef {
                key: "WINE_CPU_TOPOLOGY",
                label: "CPU 拓扑（如 4:8）",
            },
        ],
    },
    EnvCategory {
        name: "音频",
        vars: &[
            EnvVarDef {
                key: "PULSE_LATENCY_MSEC",
                label: "PulseAudio 延迟（ms）",
            },
            EnvVarDef {
                key: "SDL_AUDIODRIVER",
                label: "SDL 音频后端（pulse/alsa/pipewire）",
            },
            EnvVarDef {
                key: "PIPEWIRE_LATENCY",
                label: "PipeWire 延迟（如 256/48000）",
            },
        ],
    },
    EnvCategory {
        name: "Vulkan",
        vars: &[
            EnvVarDef {
                key: "VK_ICD_FILENAMES",
                label: "Vulkan ICD 文件路径",
            },
            EnvVarDef {
                key: "AMD_VULKAN_ICD",
                label: "AMD 驱动选择（RADV/AMDVLK）",
            },
        ],
    },
    EnvCategory {
        name: "Mesa / OpenGL",
        vars: &[
            EnvVarDef {
                key: "MESA_GL_VERSION_OVERRIDE",
                label: "OpenGL 版本（如 4.6）",
            },
            EnvVarDef {
                key: "MESA_GLSL_VERSION_OVERRIDE",
                label: "GLSL 版本（如 460）",
            },
            EnvVarDef {
                key: "MESA_NO_ERROR",
                label: "禁用 GL 错误检查（0/1）",
            },
            EnvVarDef {
                key: "mesa_glthread",
                label: "Mesa 多线程（true/false）",
            },
            EnvVarDef {
                key: "MESA_LOADER_DRIVER_OVERRIDE",
                label: "Mesa 驱动（radeonsi/iris/zink）",
            },
        ],
    },
    EnvCategory {
        name: "NVIDIA",
        vars: &[
            EnvVarDef {
                key: "__GL_THREADED_OPTIMIZATIONS",
                label: "OpenGL 多线程（0/1）",
            },
            EnvVarDef {
                key: "__GL_SHADER_DISK_CACHE",
                label: "着色器缓存（0/1）",
            },
            EnvVarDef {
                key: "__GL_SHADER_DISK_CACHE_PATH",
                label: "着色器缓存路径",
            },
            EnvVarDef {
                key: "WINE_HIDE_NVIDIA_GPU",
                label: "隐藏 NVIDIA GPU（1=隐藏）",
            },
            EnvVarDef {
                key: "__NV_PRIME_RENDER_OFFLOAD",
                label: "Prime 渲染卸载（1=启用）",
            },
            EnvVarDef {
                key: "__GLX_VENDOR_LIBRARY_NAME",
                label: "GLX 提供方（nvidia/mesa）",
            },
        ],
    },
    EnvCategory {
        name: "DLSS / FSR / XeSS",
        vars: &[
            EnvVarDef {
                key: "PROTON_DLSS_UPGRADE",
                label: "DLSS 自动更新（0/1）",
            },
            EnvVarDef {
                key: "PROTON_DLSS_INDICATOR",
                label: "DLSS 指示器（0/1）",
            },
            EnvVarDef {
                key: "PROTON_FSR4_UPGRADE",
                label: "FSR4 自动更新 - Proton-CachyOS（0/1）",
            },
            EnvVarDef {
                key: "FSR4_UPGRADE",
                label: "FSR4 自动更新 - GE-Proton（0/1）",
            },
            EnvVarDef {
                key: "PROTON_FSR4_RDNA3_UPGRADE",
                label: "RDNA3 FSR4（0/1）",
            },
            EnvVarDef {
                key: "PROTON_XESS_UPGRADE",
                label: "XeSS 自动更新（0/1）",
            },
        ],
    },
    EnvCategory {
        name: "性能监控",
        vars: &[
            EnvVarDef {
                key: "GALLIUM_HUD",
                label: "Gallium3D HUD（fps、cpu 等）",
            },
            EnvVarDef {
                key: "MANGOHUD",
                label: "MangoHud（0=禁用，1=启用）",
            },
            EnvVarDef {
                key: "MANGOHUD_CONFIG",
                label: "MangoHud 配置",
            },
            EnvVarDef {
                key: "MANGOHUD_CONFIGFILE",
                label: "MangoHud 配置文件路径",
            },
        ],
    },
    EnvCategory {
        name: "输入设备",
        vars: &[
            EnvVarDef {
                key: "PROTON_PREFER_SDL",
                label: "优先使用 SDL 输入（0/1）",
            },
            EnvVarDef {
                key: "SDL_GAMECONTROLLERCONFIG",
                label: "手柄映射配置",
            },
            EnvVarDef {
                key: "SDL_JOYSTICK_DEVICE",
                label: "输入设备（如 js0）",
            },
        ],
    },
    EnvCategory {
        name: "字体",
        vars: &[EnvVarDef {
            key: "FREETYPE_PROPERTIES",
            label: "FreeType 渲染配置",
        }],
    },
    EnvCategory {
        name: "其他",
        vars: &[
            EnvVarDef {
                key: "COPYPREFIX",
                label: "复制前缀（0/1）",
            },
            EnvVarDef {
                key: "SteamDeck",
                label: "SteamDeck 模式（0/1）",
            },
            EnvVarDef {
                key: "PROTON_ADD_CONFIG",
                label: "快捷参数（如 fsr4,wayland,hdr）",
            },
            EnvVarDef {
                key: "WINE_DISABLE_FAST_SYNC",
                label: "禁用快速同步（0/1）",
            },
            EnvVarDef {
                key: "WINE_DISABLE_WRITE_WATCH",
                label: "禁用写入检测（0/1）",
            },
            EnvVarDef {
                key: "STAGING_SHARED_MEMORY",
                label: "共享内存（0/1）",
            },
            EnvVarDef {
                key: "WINEVERPATH",
                label: "Wine 版本路径",
            },
            EnvVarDef {
                key: "WINE",
                label: "Wine 可执行文件路径",
            },
        ],
    },
];

/// 是否为 0/1 开关型变量。
pub fn is_bool_toggle(key: &str) -> bool {
    BOOL_TOGGLE_KEYS.contains(&key)
}

/// 是否需要在值输入行旁提供「选择文件」按钮。
pub fn is_file_key(key: &str) -> bool {
    FILE_KEYS.contains(&key)
}

/// 是否需要在值输入行旁提供「选择目录」按钮。
pub fn is_dir_key(key: &str) -> bool {
    DIR_KEYS.contains(&key)
}

/// 是否为内置（非自定义）变量。
pub fn is_known_key(key: &str) -> bool {
    EXTRA_KNOWN_KEYS.contains(&key)
        || ENV_CATEGORIES
            .iter()
            .any(|c| c.vars.iter().any(|v| v.key == key))
}

/// 变量是否处于启用状态（内置开关型按 "1"/"0" 语义判定）。
pub fn is_enabled(key: &str, value: &str) -> bool {
    if is_bool_toggle(key) {
        !matches!(value, "0" | "false" | "False" | "no")
    } else {
        true
    }
}

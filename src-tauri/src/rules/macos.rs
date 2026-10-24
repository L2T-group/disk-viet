//! macOS rule catalog (spec §6). Patterns are case-sensitive: casing must match macOS exactly.
use super::Cleaner::{self, *};
use super::Rule;
use super::Safety::{self, *};

const fn r(id: &'static str, patterns: &'static [&'static str], safety: Safety) -> Rule {
    Rule {
        id,
        patterns,
        safety,
        needs_admin: false,
        processes: &[],
        cleaner: Files,
        in_repo: false,
    }
}

const fn quit(rule: Rule, processes: &'static [&'static str]) -> Rule {
    Rule { processes, ..rule }
}

const fn admin(rule: Rule) -> Rule {
    Rule {
        needs_admin: true,
        ..rule
    }
}

const fn tool(rule: Rule, cleaner: Cleaner) -> Rule {
    Rule { cleaner, ..rule }
}

const fn repo(rule: Rule) -> Rule {
    Rule {
        in_repo: true,
        ..rule
    }
}

pub static RULES: &[Rule] = &[
    // 🟢 Safe
    r(
        "xcode_derived_data",
        &["~/Library/Developer/Xcode/DerivedData"],
        Safe,
    ),
    admin(r(
        "simulator_caches",
        &["/Library/Developer/CoreSimulator/Caches"],
        Safe,
    )),
    r(
        "ios_device_support",
        &["~/Library/Developer/Xcode/iOS DeviceSupport"],
        Safe,
    ),
    quit(
        r(
            "telegram_cache",
            &[
                "~/Library/Caches/ru.keepcoder.Telegram",
                "~/Library/Group Containers/*.ru.keepcoder.Telegram/**/postbox/media",
            ],
            Safe,
        ),
        &["Telegram"],
    ),
    quit(
        r("chrome_cache", &["~/Library/Caches/Google"], Safe),
        &["Google Chrome"],
    ),
    quit(
        r("safari_cache", &["~/Library/Caches/com.apple.Safari"], Safe),
        &["Safari"],
    ),
    quit(
        r("firefox_cache", &["~/Library/Caches/Firefox"], Safe),
        &["firefox"],
    ),
    quit(
        r("brave_cache", &["~/Library/Caches/BraveSoftware"], Safe),
        &["Brave Browser"],
    ),
    quit(
        r("edge_cache", &["~/Library/Caches/Microsoft Edge"], Safe),
        &["Microsoft Edge"],
    ),
    r("jetbrains_cache", &["~/Library/Caches/JetBrains"], Safe),
    quit(
        r(
            "cursor_cache",
            &[
                "~/Library/Application Support/Cursor/{Cache,CachedData,CachedExtensionVSIXs,GPUCache}",
            ],
            Safe,
        ),
        &["Cursor"],
    ),
    quit(
        r(
            "vscode_cache",
            &[
                "~/Library/Application Support/Code/{Cache,CachedData,CachedExtensionVSIXs,GPUCache}",
            ],
            Safe,
        ),
        &["Code"],
    ),
    tool(
        r("homebrew_cache", &["~/Library/Caches/Homebrew"], Safe),
        BrewCleanup,
    ),
    r(
        "node_pkg_cache",
        &[
            "~/.npm/_cacache",
            "~/Library/Caches/pnpm",
            "~/Library/pnpm/store",
            "~/Library/Caches/Yarn",
        ],
        Safe,
    ),
    r(
        "python_pkg_cache",
        &["~/Library/Caches/pip", "~/.cache/uv"],
        Safe,
    ),
    r("cocoapods_cache", &["~/Library/Caches/CocoaPods"], Safe),
    r("logs", &["~/Library/Logs/*"], Safe),
    // 🟡 Caution
    // Not 🔑: simctl removes the image itself, run as the user. Only the image (`.dmg`) or its
    // `.asset` dir is a target: the mounted `CoreSimulator/Volumes/*` are mount roots, which
    // the guard refuses (`mount_root`).
    tool(
        r(
            "simulator_runtimes",
            &[
                "/Library/Developer/CoreSimulator/Images/*.dmg",
                // Inside /System only because the cleaner is SimctlRuntime, never a plain file delete.
                "/System/Library/AssetsV2/com_apple_MobileAsset_*SimulatorRuntime/*.asset",
            ],
            Caution,
        ),
        SimctlRuntime,
    ),
    tool(
        r(
            "simulator_devices",
            &["~/Library/Developer/CoreSimulator/Devices"],
            Caution,
        ),
        SimctlDeleteUnavailable,
    ),
    r(
        "gradle_cache",
        &["~/.gradle/caches", "~/.gradle/wrapper/dists"],
        Caution,
    ),
    r("android_avd", &["~/.android/avd/*.avd"], Caution),
    r(
        "android_sdk",
        &["~/Library/Android/sdk/{system-images,ndk,build-tools,platforms,sources,emulator}"],
        Caution,
    ),
    r("dart_cache", &["~/.pub-cache", "~/.dartServer"], Caution),
    r("expo_cache", &["~/.expo"], Caution),
    repo(r("repo_node_modules", &["**/node_modules"], Caution)),
    repo(r(
        "repo_build_output",
        &["**/{target,build,.dart_tool}"],
        Caution,
    )),
    repo(r("repo_pods", &["**/Pods"], Caution)),
    quit(
        r(
            "cursor_global_storage",
            &["~/Library/Application Support/Cursor/User/globalStorage"],
            Caution,
        ),
        &["Cursor"],
    ),
    tool(
        r(
            "docker_data",
            &["~/Library/Containers/com.docker.docker"],
            Caution,
        ),
        DockerPrune,
    ),
    r("downloads", &["~/Downloads"], Caution),
    r(
        "xcode_archives",
        &["~/Library/Developer/Xcode/Archives"],
        Caution,
    ),
    r("trash", &["~/.Trash"], Caution),
    quit(
        r(
            "zalo_data",
            &["~/Library/Application Support/ZaloData"],
            Caution,
        ),
        &["Zalo"],
    ),
    quit(
        r(
            "telegram_data",
            &["~/Library/Group Containers/*.ru.keepcoder.Telegram"],
            Caution,
        ),
        &["Telegram"],
    ),
    // 🔴 Important
    r(
        "user_documents",
        &["~/{Documents,Desktop,Pictures,Movies,Music}/*"],
        Important,
    ),
    r("photos_library", &["~/Pictures/*.photoslibrary"], Important),
    r(
        "iphone_backups",
        &["~/Library/Application Support/MobileSync/Backup"],
        Important,
    ),
    r("mail_messages", &["~/Library/{Mail,Messages}"], Important),
    // Matched by `has_git`, not by pattern (see `classify`).
    r("git_projects", &[], Important),
    r(
        "dev_config",
        &["~/{.gitconfig,.zshrc,.zprofile,.bashrc,.bash_profile,.config}"],
        Important,
    ),
    // ⚫ System
    r(
        "system_files",
        &[
            "/System",
            "/usr/{bin,lib,libexec,sbin,share,standalone}",
            "/bin",
            "/sbin",
            "/private/var/db",
            "/Library/Apple",
        ],
        System,
    ),
    r("swap", &["/private/var/vm", "/System/Volumes/VM"], System),
    // SIP `restricted`: even root gets EPERM; macOS manages these itself.
    r("macos_updates", &["/Library/Updates"], System),
    r("applications", &["/Applications"], System),
    r("keychains", &["~/Library/Keychains"], System),
    r("ssh_gpg", &["~/.ssh", "~/.gnupg"], System),
    r("preferences", &["~/Library/Preferences"], System),
];

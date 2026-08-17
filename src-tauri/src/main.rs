// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    {
        // main() içinde, app build'den ÖNCE tek iş parçacıklı ortamda çağrılmalı.
        // (set_var Rust 2024'te unsafe olur; bu proje edition 2021 kullanıyor.)
        // Kullanıcı kendi değerini export etmişse ezme.
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1"); // NVIDIA+Wayland GBM "Error 71" koruması
        }
        if std::env::var_os("__NV_DISABLE_EXPLICIT_SYNC").is_none() {
            std::env::set_var("__NV_DISABLE_EXPLICIT_SYNC", "1");
        }
    }

    oh_my_opencode_slim_companion_lib::run()
}
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use fs_extra::{
    dir::get_size,
    file::{copy_with_progress, CopyOptions, TransitProcess},
};
use std::{fs, path::Path};
use tauri_plugin_shell::process::CommandEvent;
use tauri::Manager;
use tauri::Emitter;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

#[tauri::command]
async fn show_help_window(handle: tauri::AppHandle) {
    if !handle.get_webview_window("help").is_none() {
        return;
    }

    let _help_window = tauri::WebviewWindowBuilder::new(
        &handle,
        "help", /* the unique window label */
        tauri::WebviewUrl::App("help".into()),
    )
    .inner_size(1024.0, 700.0)
    .min_inner_size(480.0, 480.0)
    .skip_taskbar(true)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .build()
    .unwrap();
}

#[tauri::command]
async fn show_youtube_modal(handle: tauri::AppHandle) {
    if !handle.get_webview_window("youtube").is_none() {
        return;
    }

    let _youtube_window = tauri::WebviewWindowBuilder::new(
        &handle,
        "youtube", /* the unique window label */
        tauri::WebviewUrl::App("youtube".into())
    )
    .inner_size(780.0, 250.0)
    .min_inner_size(480.0, 200.0)
    .resizable(true)
    .skip_taskbar(true)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .build()
    .unwrap();
}

#[derive(Clone, serde::Serialize)]
struct Payload {
    path: String,
    code: String,
}

#[tauri::command]
async fn clear_cache(_handle: tauri::AppHandle) -> String {
    // Get path to youtube_downloads foldre
    let app_data_dir = dirs::cache_dir().unwrap();
    let downloads_folder = Path::new(&app_data_dir).join("youtube_downloads");

    // Create folder if it doesn't exist
    if !downloads_folder.is_dir() {
        fs::create_dir_all(&downloads_folder).expect("Could not create downloads folder");
        return String::from("");
    } else {
        let size = get_size(&downloads_folder).unwrap();

        if size <= 0 {
            return String::from("EMPTY");
        }

        fs::remove_dir_all(&downloads_folder).expect("Could not delete downloads folder");
        fs::create_dir_all(&downloads_folder).expect("Could not recreate downloads folder");

        return String::from(format!("{}", size));
    }
}

#[tauri::command]
async fn download_video(
    handle: tauri::AppHandle,
    url: String,
    code: String,
    quality: String,
) -> String {
    // Get path to youtube_downloads foldre
    let app_data_dir = dirs::cache_dir().unwrap();
    let downloads_folder = Path::new(&app_data_dir).join("youtube_downloads");

    // Create folder if it doesn't exist
    if !downloads_folder.is_dir() {
        fs::create_dir_all(&downloads_folder).expect("Could not create downloads folder");
    }

    let mut vid_path: String = String::from("");
    let glob_pattern = format!("{}/youtube_downloads/*{}*", app_data_dir.display(), code);

    for entry in glob::glob(&glob_pattern).expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => {
                vid_path = path.display().to_string();
                break;
            }
            Err(e) => println!("{:?}", e),
        }
    }

    if vid_path.len() > 0 {
        println!("Getting video from cache");

        let _ = handle
            .get_webview_window("main")
            .unwrap()
            .emit(
                "video-downloaded",
                Payload {
                    path: vid_path,
                    code: code,
                },
            )
            .expect("");

        return "".to_string();
    }

    let quality_code;

    match quality.as_str() {
        "1" => quality_code = "137+251/136+251/135+251/134+251",
        "2" => quality_code = "136+251/135+251/134+251",
        "3" => quality_code = "135+251/134+251",
        "4" => quality_code = "134+251",
        "5" => quality_code = "251",
        _ => quality_code = "137+251/136+251/135+251/134+251",
    }

    let (mut rx, mut _child) = handle.shell().sidecar("yt-dlp")
        .expect("Failed to create yt-dlp binary command")
        .args([
            url,
            "-P".to_string(),
            downloads_folder.display().to_string(),
            "--extractor-args".to_string(),
            "youtube:skip=hls,dash;youtube:skip=translated_subs".to_string(),
            "-f".to_string(),
            quality_code.to_string(),
            "--print".to_string(),
            "after_move:filepath".to_string(),
            "--no-simulate".to_string(),
        ])
        .spawn()
        .expect("Failed to spawn yt-dlp");

    let mut video_path: String = String::from("");

    let result = tauri::async_runtime::spawn(async move {
        // read events such as stdout
        while let Some(event) = rx.recv().await {
            if let CommandEvent::Stdout(line) = event {
                video_path = String::from_utf8(line).expect("Our bytes should be valid utf 8");
            }
        }

        // Clear any trailing \n or \r\n
        if video_path.ends_with('\n') {
            video_path.pop();
            if video_path.ends_with('\r') {
                video_path.pop();
            }
        }

        if video_path.is_empty() || (!Path::new(&downloads_folder).join(&video_path).exists()) {
            return "Failed to download video";
        }

        let _ = handle
            .get_webview_window("main")
            .unwrap()
            .emit(
                "video-downloaded",
                Payload {
                    path: video_path,
                    code: code,
                },
            )
            .expect("");

        return "";
    })
    .await;

    result.unwrap().to_string()
}

#[tauri::command]
async fn save_youtube_video(_handle: AppHandle, code: String, path_to_save: String) -> String {
    // Get path to youtube_downloads folder
    let app_data_dir = dirs::cache_dir().unwrap();
    let downloads_folder = Path::new(&app_data_dir).join("youtube_downloads");

    // Create folder if it doesn't exist
    if !downloads_folder.is_dir() {
        fs::create_dir_all(&downloads_folder).expect("Could not create downloads folder");
    }

    let mut vid_path: String = String::from("");
    let glob_pattern = format!("{}/youtube_downloads/*{}*", app_data_dir.display(), code);

    for entry in glob::glob(&glob_pattern).expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => {
                vid_path = path.display().to_string();
                break;
            }
            Err(e) => println!("{:?}", e),
        }
    }

    if vid_path.len() > 0 {
        let options = CopyOptions::new();
        let progress_handler =
            |process_info: TransitProcess| println!("{}", process_info.copied_bytes);

        let _ = copy_with_progress(vid_path, path_to_save, &options, progress_handler);

        return String::from("");
    } else {
        return String::from("Currently playing video has been deleted");
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_cli::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            show_help_window,
            show_youtube_modal,
            download_video,
            clear_cache,
            save_youtube_video
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

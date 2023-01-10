use serde::Deserialize;
use sqlx::SqlitePool;
use std::fs::{create_dir_all, read_dir};
use std::path::{Path, PathBuf};
use std::process::Command;

pub mod admin;
pub mod album;
pub mod index;
pub mod post;
pub mod upload;

pub struct AppState {
    pool: SqlitePool,
    ipp: i64,
    root_dir: PathBuf,
    thumbnail_dir: PathBuf,
}

impl AppState {
    pub fn new(pool: SqlitePool, ipp: i64, root_dir: PathBuf, thumbnail_dir: PathBuf) -> Self {
        AppState {
            pool,
            ipp,
            root_dir,
            thumbnail_dir,
        }
    }
}

#[derive(Deserialize)]
pub struct QueryInfo {
    page: Option<u32>,
    id: Option<i64>,
    view: Option<String>,
    tags: Option<String>,
    file_name: Option<String>,
    real_file_name: Option<String>,
    md5: Option<String>,
    raw: Option<u8>,
}

macro_rules! redirect {
    ($url: expr) => {
        HttpResponse::Found()
            .append_header(("Location", $url))
            .finish()
    };
}
pub(crate) use redirect;

fn guess_file_type(file_name: &str) -> &str {
    let parts: Vec<&str> = file_name.split(".").collect();
    match parts.last() {
        Some(v) => match *v {
            "png" | "jpeg" | "jpg" | "gif" | "webp" | "bmp" | "PNG" | "JPG" | "JPEG" | "GIF"
            | "WEBP" | "BMP" => "image",
            "mp4" | "mpg" | "webm" | "mkv" | "avi" | "mts" | "flv" | "m3u8" | "MP4" | "MPG"
            | "WEBM" | "MKV" | "AVI" | "MTS" | "FLV" | "M3U8" => "video",
            _ => "unknown",
        },
        None => "unknown",
    }
}

fn create_thumbnail(
    root_dir: &str,
    thumbnail_dir: &str,
    file_path: &str,
    file_type: &str,
    force: bool,
) {
    let thumb_path_wo_ext = file_path.replacen(root_dir, &format!("{}/", thumbnail_dir), 1);
    let thumb_path = format!("{}.jpg", thumb_path_wo_ext);
    let thumb_file = Path::new(&thumb_path);
    if force || file_type == "folder" || !thumb_file.exists() {
        let thumb_file_parrent = thumb_file.parent().unwrap();
        if !thumb_file_parrent.exists() {
            if let Err(_) = create_dir_all(&thumb_file_parrent) {
                return;
            };
        }

        if file_type == "image" {
            Command::new("convert")
                .args([
                    "-quiet",
                    "-thumbnail",
                    "300",
                    &format!("{}[0]", file_path),
                    &thumb_path,
                ])
                .status()
                .expect("Failed to create thumbnail");
        } else if file_type == "video" {
            Command::new("ffmpeg")
                .args([
                    "-y",
                    "-loglevel",
                    "quiet",
                    "-i",
                    file_path,
                    "-frames",
                    "15",
                    "-vf",
                    r#"select=not(mod(n\,3000)),scale=300:ih*300/iw"#,
                    "-q:v",
                    "10",
                    &thumb_path,
                ])
                .status()
                .expect("Failed to create thumbnail");
        } else if file_type == "folder" {
            let mut args = vec![
                "-tile".to_string(),
                "2x2".to_string(),
                "-quality".to_string(),
                "-25".to_string(),
                "-geometry".to_string(),
                "+1+1".to_string(),
            ];
            let mut count = 0;
            let folder_path = Path::new(&thumb_path_wo_ext);
            if !folder_path.exists() || !folder_path.is_dir() {
                return;
            }
            for i in read_dir(folder_path).unwrap() {
                args.push(i.unwrap().path().to_str().unwrap().to_string());
                count += 1;
                if count >= 4 {
                    break;
                }
            }

            args.push(thumb_path);
            Command::new("montage")
                .args(args)
                .status()
                .expect("Failed to create folder thumbnail");
        }
    }
}

mod db;
mod route;

use actix_files::Files;
use actix_web::web::Data;
use actix_web::{App, HttpServer};
use clap::Parser;
use configparser::ini::Ini;
use dotenv::dotenv;
use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;
use tera::Tera;

#[derive(Parser)]
struct Cli {
    #[clap(short, long, default_value = "config.ini")]
    config: String,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    let args = Cli::parse();
    let mut config = Ini::new();
    let _ = config.load(args.config);
    let config_root_dir = config.get("default", "root").unwrap();
    let root_dir = Path::new(&config_root_dir).to_path_buf();
    let thumbnail_dir = root_dir.join("thumbnail");
    let db_path = config.get("default", "db").unwrap();
    let port = config.get("default", "port").unwrap();
    let ipp: u32 = config
        .get("default", "ipp")
        .unwrap_or("48".to_owned())
        .parse()
        .unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_path)
        .await
        .unwrap();

    HttpServer::new(move || {
        let tera = Tera::new(concat!(env!("CARGO_MANIFEST_DIR"), "/res/html/**/*")).unwrap();

        App::new()
            .app_data(Data::new(tera))
            .app_data(Data::new(route::AppState::new(
                pool.clone(),
                ipp as i64,
                root_dir.clone(),
                thumbnail_dir.clone(),
            )))
            .service(route::index::index)
            .service(route::admin::admin)
            .service(route::admin::manage_tags)
            .service(route::admin::manage_tag)
            .service(route::admin::tag_update)
            .service(route::admin::tag_delete)
            .service(route::admin::reload)
            .service(route::post::item_update)
            .service(route::post::delete)
            .service(route::upload::upload)
            .service(route::upload::upload_item)
            .service(route::upload::post_upload)
            .service(route::album::get_new)
            .service(route::album::post_new)
            .service(Files::new("/img", root_dir.clone()))
            .service(Files::new(
                "/css",
                concat!(env!("CARGO_MANIFEST_DIR"), "/res/css"),
            ))
            .service(Files::new(
                "/js",
                concat!(env!("CARGO_MANIFEST_DIR"), "/res/js"),
            ))
    })
    .bind(format!("0.0.0.0:{}", port))?
    .run()
    .await
}

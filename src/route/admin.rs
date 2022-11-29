use actix_web::{error, get, post, web, HttpResponse, Responder};
use md5::{Digest, Md5};
use serde::Deserialize;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use walkdir::WalkDir;

use super::{create_thumbnail, guess_file_type, redirect, AppState};
use crate::db::{item, tag};

#[derive(Deserialize)]
pub struct TagData {
    id: Option<i64>,
    name: Option<String>,
    deps: Option<String>,
}

#[get("/admin/")]
pub async fn admin(tmpl: web::Data<tera::Tera>) -> impl Responder {
    let ctx = tera::Context::new();
    let template = tmpl
        .render("admin.html", &ctx)
        .map_err(|_| error::ErrorInternalServerError("Template error"))
        .unwrap();
    HttpResponse::Ok().content_type("text/html").body(template)
}

#[get("/admin/tags/")]
pub async fn manage_tags(data: web::Data<AppState>, tmpl: web::Data<tera::Tera>) -> impl Responder {
    let mut ctx = tera::Context::new();
    if let Ok(tags) = tag::count_tags(&data.pool).await {
        ctx.insert("tags", &tags);
    }
    let template = tmpl
        .render("tags.html", &ctx)
        .map_err(|_| error::ErrorInternalServerError("Template error"))
        .unwrap();
    HttpResponse::Ok().content_type("text/html").body(template)
}

#[get("/admin/tag/{name}")]
pub async fn manage_tag(
    data: web::Data<AppState>,
    name: web::Path<String>,
    tmpl: web::Data<tera::Tera>,
) -> impl Responder {
    let mut ctx = tera::Context::new();

    let tag = tag::find_or_create(&data.pool, &name.into_inner())
        .await
        .unwrap();
    let deps = tag::find_depend_tags(&data.pool, tag.id).await.unwrap();

    ctx.insert("tag", &tag);
    ctx.insert("deps", &deps);
    let template = tmpl
        .render("tag.html", &ctx)
        .map_err(|_| error::ErrorInternalServerError("Template error"))
        .unwrap();
    HttpResponse::Ok().content_type("text/html").body(template)
}

#[post("/admin/tag/")]
pub async fn tag_update(data: web::Data<AppState>, tagdata: web::Form<TagData>) -> impl Responder {
    let name = tagdata.name.as_ref().unwrap();
    let id = tagdata.id.unwrap();
    if let Ok(tag) = tag::find_by_name(&data.pool, name).await {
        if tag.id != id {
            eprintln!("Tag with name {} already exists", name);
            return redirect!(format!("/admin/tag/{}", name));
        }
    }

    let mut deps: Vec<&str> = Vec::new();
    if let Some(post_deps) = &tagdata.deps {
        deps = post_deps.split_whitespace().collect();
    }

    tag::update_tag(&data.pool, id, &name, deps).await;
    redirect!(format!("/admin/tag/{}", name))
}

#[get("/admin/reload/")]
pub async fn reload(data: web::Data<AppState>) -> impl Responder {
    for entry in WalkDir::new(&data.root_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let file_path = entry.path().to_str().unwrap();
        if entry.path().starts_with(Path::new(&data.thumbnail_dir)) {
            continue;
        }
        let root_path = Path::new(&data.root_dir);
        // let rel_path = file_path.replacen(&data.root_dir, "", 1);
        let rel_path = entry
            .path()
            .strip_prefix(root_path)
            .unwrap()
            .to_str()
            .unwrap_or("");
        if rel_path == "" {
            continue;
        }
        let file_name = entry.file_name().to_str().unwrap();
        let mut file_type = if entry.path().is_dir() {
            "folder"
        } else {
            guess_file_type(file_name)
        };

        if file_type == "video" {
            if entry.metadata().unwrap().len() < 5242880 {
                file_type = "video/short";
            }
        }

        let mut item = match item::find_by_path(&data.pool, rel_path).await {
            Ok(_item) => _item,
            Err(_) => item::Item::new(
                file_name.to_owned(),
                rel_path.to_string(),
                file_type.to_owned(),
            ),
        };

        let force = if file_type == "folder" { true } else { false };

        create_thumbnail(
            data.root_dir.to_str().unwrap(),
            data.thumbnail_dir.to_str().unwrap(),
            file_path,
            file_type,
            force,
        );

        let parent_folder = Path::new(&item.path)
            .parent()
            .unwrap_or(Path::new(""))
            .to_str()
            .unwrap();
        if !parent_folder.is_empty() {
            if let Ok(_item) = item::find_by_path(&data.pool, parent_folder).await {
                item.parent = Some(_item.id);
            } else {
                item.parent = None;
            }
        }

        if item.id == 0 && file_type != "unknown" {
            let mut md5 = Md5::new();
            if file_type == "folder" {
                md5.update(item.path.as_str());
            } else {
                if let Ok(mut file) = File::open(&file_path) {
                    let mut reader = BufReader::new(file);
                    let mut buffer = [0; 1024];
                    loop {
                        let count = reader.read(&mut buffer).unwrap_or_default();
                        if count == 0 {
                            break;
                        }
                        md5.update(&buffer[..count]);
                    }
                }
            }
            item.md5 = format!("{:x}", md5.finalize());
            match item::find_by_md5(&data.pool, &item.md5).await {
                Ok(_) => {
                    println!("{}: duplicated md5sum {}.", item.path, item.md5);
                    item::delete_local_file(file_path).await;
                    item::delete_local_file(&format!(
                        "{}/{}.jpg",
                        data.thumbnail_dir.to_str().unwrap(),
                        file_path
                    ))
                    .await;
                }
                Err(_) => {
                    if let Err(err) = item::insert(&data.pool, &item).await {
                        eprintln!("Failed to insert item. {:?}", err);
                    }
                }
            };
        } else {
            if item.file_type != file_type {
                item.file_type = file_type.to_string();
                item::update(&data.pool, item).await;
            }
        }
    }

    redirect!("/admin/")
}

#[get("/delete/tag/{id}")]
pub async fn tag_delete(data: web::Data<AppState>, id: web::Path<i64>) -> impl Responder {
    tag::delete_tag(&data.pool, id.into_inner()).await;
    redirect!("/admin/tags/")
}

use actix_multipart::Multipart;
use actix_web::{error, get, post, web, HttpResponse, Responder};
use async_std::io::WriteExt;
use futures::{StreamExt, TryStreamExt};
use md5::{Digest, Md5};
use std::fs::{create_dir_all, rename};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::post::PostData;
use super::{create_thumbnail, guess_file_type, redirect, AppState, QueryInfo};
use crate::db::{item, tag};

#[get("/upload/")]
pub async fn upload(
    data: web::Data<AppState>,
    tmpl: web::Data<tera::Tera>,
    query: web::Query<QueryInfo>,
) -> impl Responder {
    let mut ctx = tera::Context::new();
    ctx.insert("post_upload", &false);
    ctx.insert("md5", query.md5.as_ref().unwrap_or(&String::new()));
    if let Some(file_name) = &query.file_name {
        ctx.insert("file_type", guess_file_type(file_name));
        if let Some(real_file_name) = &query.real_file_name {
            ctx.insert("file_name", &file_name);
            ctx.insert("real_file_name", &real_file_name);
            ctx.insert("post_upload", &true);
        }
    }

    let folders = item::find_by_type(&data.pool, "folder")
        .await
        .unwrap_or(vec![]);
    ctx.insert("parents", &folders);

    let all_tags = tag::find_all(&data.pool).await.unwrap_or(vec![]);
    ctx.insert("tags", &all_tags);

    let template = tmpl
        .render("upload.html", &ctx)
        .map_err(|_| error::ErrorInternalServerError("Template error"))
        .unwrap();
    HttpResponse::Ok().content_type("text/html").body(template)
}

#[post("/upload/")]
pub async fn upload_item(data: web::Data<AppState>, mut payload: Multipart) -> impl Responder {
    let tmp_dir_path = Path::new(&data.root_dir).join("tmp");
    if !tmp_dir_path.exists() {
        if let Err(_) = create_dir_all(&tmp_dir_path) {
            return redirect!("/");
        };
    }

    // TODO: Upload multiple files
    let mut file_name = String::new();
    let mut real_file_name = String::new();

    while let Ok(Some(mut field)) = payload.try_next().await {
        let mut md5_context = Md5::new();
        let mut md5sum = String::new();

        file_name = field
            .content_disposition()
            .get_filename()
            .ok_or_else(|| actix_web::error::ParseError::Incomplete)
            .unwrap()
            .to_string();
        real_file_name = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros()
            .to_string();
        let parts: Vec<&str> = file_name.split(".").collect();
        let mut ext = "png";
        if let Some(_ext) = parts.last() {
            real_file_name.push_str(&format!(".{}", *_ext));
            ext = _ext;
        } else {
            real_file_name.push_str("png");
        }
        // let filepath = format!("{}/{}", tmp_dir, sanitize_filename::sanitize(&filename));
        let file_path = tmp_dir_path.join(&real_file_name);
        let mut f = async_std::fs::File::create(file_path.to_str().unwrap())
            .await
            .unwrap();

        // Field in turn is stream of *Bytes* object
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            md5_context.update(&data);
            f.write_all(&data).await;
        }

        md5sum = format!("{:x}", md5_context.finalize());

        if let Ok(item) = item::find_by_md5(&data.pool, &md5sum).await {
            println!("File existed: {}", item.path);
            redirect!("/upload/");
        }

        let new_file_name = format!("{}.{}", md5sum, ext);
        let new_file_path = tmp_dir_path.join(&new_file_name);
        if let Ok(_) = rename(&file_path, &new_file_path) {
            return redirect!(format!(
                "/upload/?file_name={}&real_file_name={}&md5={}",
                file_name, new_file_name, md5sum
            ));
        }
    }

    redirect!("/")
}

#[post("/post_upload/")]
pub async fn post_upload(data: web::Data<AppState>, form: web::Form<PostData>) -> impl Responder {
    let mut dest_dir = data.root_dir.clone();
    let mut item = item::Item::empty();
    if let Some(parent) = &form.parent {
        if let Ok(parent_id) = parent.parse::<i64>() {
            if let Ok(parent_item) = item::find_by_id(&data.pool, parent_id).await {
                if parent_item.file_type == "folder" {
                    dest_dir = data.root_dir.join(Path::new(&parent_item.path));
                    item.parent = Some(parent_item.id);
                }
            }
        }
    }
    if let Some(real_file_name) = &form.real_name {
        let tmp_file = data.root_dir.join("tmp").join(real_file_name);
        let dest_file = dest_dir.join(real_file_name);
        if let Ok(()) = rename(&tmp_file, &dest_file) {
            let file_name = match &form.name {
                Some(name) => name,
                None => real_file_name,
            };

            item.name = file_name.to_string();
            item.path = dest_file
                .strip_prefix(data.root_dir.as_path())
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            item.file_type = guess_file_type(real_file_name).to_string();
            item.md5 = form.md5.as_ref().unwrap().clone();
            if let Ok(id) = item::insert(&data.pool, &item).await {
                if let Some(_tags) = &form.tags {
                    let tags: Vec<&str> = _tags.split_whitespace().collect();
                    tag::update_item_tags(&data.pool, id, tags).await;
                }

                create_thumbnail(
                    data.root_dir.to_str().unwrap(),
                    data.thumbnail_dir.to_str().unwrap(),
                    dest_file.to_str().unwrap(),
                    &item.file_type,
                    false,
                );
                return redirect!(format!("/?id={}", id));
            }
        }
    }

    redirect!("/upload/")
}

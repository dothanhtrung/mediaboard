use db::*;

mod db;

use std::collections::HashMap;
use std::fs::{create_dir_all, rename, File, read_dir};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use actix_files::Files;
use actix_multipart::Multipart;
use actix_web::{App, error, get, HttpResponse, HttpServer, post, Responder, web};
use async_std::prelude::*;
use configparser::ini::Ini;
use futures::{StreamExt, TryStreamExt};
use md5::{Md5, Digest};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use tera::Tera;
use walkdir::WalkDir;

struct AppState {
    conn: Connection,
    ipp: u64,
    root_dir: PathBuf,
    thumbnail_dir: PathBuf,
}

#[derive(Deserialize)]
struct QueryInfo {
    page: Option<u64>,
    id: Option<i64>,
    view: Option<String>,
    tags: Option<String>,
    file_name: Option<String>,
    real_file_name: Option<String>,
    md5: Option<String>,
}

#[derive(Deserialize)]
struct PostData {
    id: Option<i64>,
    tags: Option<String>,
    name: Option<String>,
    real_name: Option<String>,
    parent: Option<String>,
    md5: Option<String>,
}


#[derive(Deserialize)]
struct TagData {
    id: Option<i64>,
    name: Option<String>,
    deps: Option<String>,
}

#[derive(Serialize)]
struct Pages {
    cur: u64,
    total: u64,
}

fn guess_file_type(file_name: &str) -> &str {
    let parts: Vec<&str> = file_name.split(".").collect();
    match parts.last() {
        Some(v) =>
            match *v {
                "png" | "jpeg" | "jpg" | "gif" | "webp" | "bmp" |
                "PNG" | "JPG" | "JPEG" | "GIF" | "WEBP" | "BMP" => "image",
                "mp4" | "mpg" | "webm" | "mkv" | "avi" | "mts" | "flv" | "m3u8" |
                "MP4" | "MPG" | "WEBM" | "MKV" | "AVI" | "MTS" | "FLV" | "M3U8" => "video",
                _ => "unknown",
            },
        None => "unknown",
    }
}

fn create_thumbnail(root_dir: &str, thumbnail_dir: &str, file_path: &str, file_type: &str, children: Vec<String>) {
    let thumb_path = format!("{}.jpg", file_path.replacen(root_dir, &format!("{}/", thumbnail_dir), 1));
    let thumb_file = Path::new(&thumb_path);
    if !thumb_file.exists() {
        let thumb_file_parrent = thumb_file.parent().unwrap();
        if !thumb_file_parrent.exists() {
            if let Err(_) = create_dir_all(&thumb_file_parrent) {
                HttpResponse::Found().header("Location", "/").finish();
            };
        }

        if file_type == "image" {
            Command::new("convert").args(["-quiet", "-thumbnail", "300", &format!("{}[0]", file_path), &thumb_path]).status().expect("Failed to create thumbnail");
        } else if file_type == "video" {
            Command::new("ffmpeg").args(["-y", "-loglevel", "quiet", "-i", file_path, "-frames", "15", "-vf", r#"select=not(mod(n\,3000)),scale=300:ih*300/iw"#, "-q:v", "10", &thumb_path]).status().expect("Failed to create thumbnail");
        } else if file_type == "folder" && children.len() > 0 {
            let mut args = Vec::new();
            args.push("-tile");
            args.push("2x2");
            args.push("-quality");
            args.push("-25");
            args.push("-geometry");
            args.push("+1+1");
            for c in children.iter() {
                args.push(c);
            }
            args.push(&thumb_path);
            Command::new("montage").args(args)
                .status().expect("Failed to create folder thumbnail");
        }
    }
}

#[get("/")]
async fn index(tmpl: web::Data<tera::Tera>, data: web::Data<AppState>, query: web::Query<QueryInfo>) -> impl Responder {
    let mut ctx = tera::Context::new();
    let mut page_tags: HashMap<String, u64> = HashMap::new();
    let mut post_tags = Vec::new();
    let mut cond = Vec::new();
    let mut where_clause = String::new();
    let mut limit_clause = format!("LIMIT {}", data.ipp);
    let join_clause = "LEFT JOIN item_tag ON item_tag.item = item.id
                       LEFT JOIN tag ON item_tag.tag = tag.id";
    let from_clause = "item";
    let group_by_clause = "GROUP BY item.id";
    let mut order_by = "ORDER BY item.created_at DESC";
    let mut old_query = Vec::new();
    let mut select_clause = String::from("item.id, item.name, item.path, item.file_type, item.created_at, item.parent, item.md5");
    let mut having_clause = "".to_owned();
    let tags_count = count_tags(&data.conn).await.unwrap_or_default();

    let folders = find_items(&data.conn, "SELECT * FROM item WHERE file_type = \"folder\"").unwrap_or(vec!());
    ctx.insert("parents", &folders);

    let mut all_tags: Vec<Tag> = Vec::new();
    if let Ok(mut tags) = find_tags(&data.conn, None).await {
        tags.sort_by(|a, b| a.name.cmp(&b.name));
        all_tags = tags;
    }
    ctx.insert("tags", &all_tags);

    let mut view = match &query.view {
        Some(v) => {
            old_query.push(format!("view={}", v));
            v.clone()
        }
        None => String::new(),
    };
    let id = query.id.unwrap_or_default();
    if id > 0 {
        old_query.push(format!("id={}", id));
        for tag in find_tags_by_items(&data.conn, vec![id]).unwrap() {
            post_tags.push(tag.name.clone());
            if view.is_empty() && tag.name == "series" {
                view = "series".to_owned();
            }
        }
        post_tags.sort_by(|a, b| a.cmp(&b));

        for tag in &post_tags {
            page_tags.insert(tag.clone(), tags_count[&tag.clone()]);
        }

        match find_item(&data.conn, &format!("id = {}", id)) {
            Ok(item) => {
                ctx.insert("item", &item);
                ctx.insert("page_tags", &page_tags);
                ctx.insert("post_tags", &post_tags);

                if item.file_type == "folder" {
                    cond.push(format!("item.parent = {}", id));
                    if view == "series" {
                        order_by = " ORDER BY item.name ASC";
                    }
                } else {
                    let template = tmpl.render("post.html", &ctx).map_err(|_| error::ErrorInternalServerError("Template error")).unwrap();
                    return HttpResponse::Ok().content_type("text/html").body(template);
                }
            }
            Err(_) => { return HttpResponse::Ok().body("Not found!"); }
        }
    } else {
        let mut searching_tags = false;
        if let Some(_search_tags) = &query.tags {
            if !_search_tags.is_empty() {
                searching_tags = true;
                select_clause.push_str(", COUNT(*) AS c");
                let search_tags: Vec<&str> = _search_tags.split_whitespace().collect();
                cond.push(format!("tag.name in (\"{}\")", search_tags.join("\",\"")));
                having_clause = format!("HAVING c = {}", search_tags.len());
                old_query.push(format!("tags={}", _search_tags));
            }
        }

        if !searching_tags {
            let find_series_sql = "SELECT item.id  FROM item
                 LEFT JOIN item_tag ON item_tag.item = item.id
                 LEFT JOIN tag ON item_tag.tag = tag.id
            WHERE tag.name == \"series\"";
            match find_item_ids(&data.conn, find_series_sql) {
                Ok(series_ids) => cond.push(format!("item.parent NOT IN ({})", series_ids.join(","))),
                Err(err) => println!("{}", err),
            }
        }
    }

    let page = query.page.unwrap_or(1);
    if page > 0 {
        limit_clause = format!("LIMIT {}, {}", (page - 1) * data.ipp, data.ipp);
    }

    if view == "album_list" {
        cond.push("item.file_type = \"folder\"".to_owned());
    }

    if !cond.is_empty() {
        if cond.len() == 1 {
            where_clause = format!("WHERE {}", cond.join(" AND "));
        } else {
            where_clause = format!("WHERE ({})", cond.join(") AND ("));
        }
    }

    let sql = format!("SELECT {} FROM {} {} {} {} {} {}", select_clause, from_clause, join_clause, where_clause, group_by_clause, having_clause, order_by);
    let count = count_items(&data.conn, &sql).unwrap_or(0);
    let mut items = match find_items(&data.conn, &format!("{} {}", sql, limit_clause)) {
        Ok(_items) => _items,
        Err(err) => {
            eprintln!("{}", err);
            Vec::new()
        }
    };

    let mut item_ids = Vec::new();
    for item in &items {
        item_ids.push(item.id);
    }
    if let Ok(_tags) = find_tags_by_items(&data.conn, item_ids) {
        for t in _tags {
            let tag_name = t.name.clone();
            if !page_tags.contains_key(&tag_name) {
                page_tags.insert(t.name, tags_count[&tag_name]);
            }
        }
    }
    // page_tags.sort_by(|a, b| a.cmp(&b));
    ctx.insert("page_tags", &page_tags);

    if id == 0 {
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    }

    let total_page = count / data.ipp + if count % data.ipp != 0 { 1 } else { 0 };
    let pages = Pages {
        cur: page,
        total: total_page,
    };

    ctx.insert("items", &items);
    ctx.insert("pages", &pages);
    ctx.insert("old_query", &old_query.join("&"));
    ctx.insert("item_id", &id);
    ctx.insert("view", &view);

    let template = tmpl.render("index.html", &ctx).map_err(|_| error::ErrorInternalServerError("Template error")).unwrap();
    HttpResponse::Ok().content_type("text/html").body(template)
}

#[post("/")]
async fn item_update(data: web::Data<AppState>, postdata: web::Form<PostData>) -> impl Responder {
    if let Some(item_id) = postdata.id {
        if let Ok(item) = find_item(&data.conn, &format!("id = {}", item_id)) {
            if let Some(_tags) = &postdata.tags {
                let tags: Vec<&str> = _tags.split_whitespace().collect();
                if let Err(err) = update_item_tags(&data.conn, item_id, tags).await {
                    eprintln!("Failed to update tag. {}", err);
                }
            }

            let name = postdata.name.as_ref().unwrap();
            let parent = postdata.parent.as_ref().unwrap();

            if let Ok(new_parent) = find_item(&data.conn, &format!("id = {}", parent)) {
                let new_parent_path = PathBuf::from(&new_parent.path);
                let item_path = Path::new(&item.path);
                let src_file = data.root_dir.join(item_path);
                let mut dest_file = PathBuf::new();
                if item.parent > 0 {
                    let old_parent = find_item(&data.conn, &format!("id = {}", item.parent)).unwrap();
                    let old_parent_path = PathBuf::from(&old_parent.path);
                    dest_file = src_file.strip_prefix(&data.root_dir).unwrap().to_path_buf();
                    dest_file = dest_file.strip_prefix(old_parent_path).unwrap().to_path_buf();
                } else {
                    dest_file = src_file.strip_prefix(&data.root_dir).unwrap().to_path_buf();
                }
                let prefix = data.root_dir.join(new_parent_path);
                dest_file = prefix.join(dest_file);

                let mut new_path = PathBuf::from(&item.path);
                if src_file != dest_file {
                    if let Err(err) = rename(src_file, &dest_file) {
                        eprintln!("Failed to move item {}. {}", item.id, err);
                    } else {
                        new_path = dest_file.strip_prefix(&data.root_dir).unwrap().to_path_buf();
                        let src_thumb = format!("{}/{}.jpg", data.thumbnail_dir.to_str().unwrap(), item.path);
                        let dest_thumb = format!("{}/{}.jpg", data.thumbnail_dir.to_str().unwrap(), new_path.to_str().unwrap());
                        let thumb_parent_path = format!("{}/{}", data.thumbnail_dir.to_str().unwrap(), new_parent.path);
                        let thumb_parent = Path::new(&thumb_parent_path);
                        if !thumb_parent.exists() {
                            create_dir_all(&thumb_parent);
                        }

                        rename(src_thumb, dest_thumb);
                    }
                }

                update_item(&data.conn, item_id, name, parent, new_path.to_str().unwrap()).await;
            }

            return HttpResponse::Found().header("Location", format!("/?id={}", item_id)).finish();
        }
    }

    HttpResponse::Found().header("Location", "/").finish()
}

#[get("/delete/{id}")]
async fn delete(data: web::Data<AppState>, web::Path(id): web::Path<i64>) -> impl Responder {
    delete_item(&data.conn, id, data.root_dir.to_str().unwrap()).await;
    HttpResponse::Found().header("Location", "/").finish()
}

#[get("/admin/")]
async fn admin(tmpl: web::Data<tera::Tera>) -> impl Responder {
    let ctx = tera::Context::new();
    let template = tmpl.render("admin.html", &ctx).map_err(|_| error::ErrorInternalServerError("Template error")).unwrap();
    HttpResponse::Ok().content_type("text/html").body(template)
}

#[get("/admin/tags/")]
async fn manage_tags(data: web::Data<AppState>, tmpl: web::Data<tera::Tera>) -> impl Responder {
    let mut ctx = tera::Context::new();
    if let Ok(mut tags) = count_tags(&data.conn).await {
        // tags.sort_by(|a, b| a.name.cmp(&b.name));
        ctx.insert("tags", &tags);
    }
    let template = tmpl.render("tags.html", &ctx).map_err(|_| error::ErrorInternalServerError("Template error")).unwrap();
    HttpResponse::Ok().content_type("text/html").body(template)
}

#[get("/admin/tag/{name}")]
async fn manage_tag(data: web::Data<AppState>, web::Path(name): web::Path<String>, tmpl: web::Data<tera::Tera>) -> impl Responder {
    let mut ctx = tera::Context::new();

    let tag = find_tag_or_create(&data.conn, &name).unwrap();
    let deps = find_depend_tags(&data.conn, tag.id).unwrap();

    ctx.insert("tag", &tag);
    ctx.insert("deps", &deps);
    let template = tmpl.render("tag.html", &ctx).map_err(|_| error::ErrorInternalServerError("Template error")).unwrap();
    HttpResponse::Ok().content_type("text/html").body(template)
}

#[post("/admin/tag/")]
async fn tag_update(data: web::Data<AppState>, tagdata: web::Form<TagData>) -> impl Responder {
    let name = tagdata.name.as_ref().unwrap();
    let id = tagdata.id.unwrap();
    if let Ok(tags) = find_tags(&data.conn, Some(&format!("name = {}", name))).await {
        if tags[0].id != id {
            eprintln!("Tag with name {} already exists", name);
            return HttpResponse::Found().header("Location", format!("/admin/tag/{}", name)).finish();
        }
    }

    let mut deps: Vec<&str> = Vec::new();
    if let Some(post_deps) = &tagdata.deps {
        deps = post_deps.split_whitespace().collect();
    }

    update_tag(&data.conn, id, &name, deps).await;
    return HttpResponse::Found().header("Location", format!("/admin/tag/{}", name)).finish();
}

#[get("/delete/tag/{id}")]
async fn tag_delete(data: web::Data<AppState>, web::Path(id): web::Path<i64>) -> impl Responder {
    delete_tag(&data.conn, id).await;
    HttpResponse::Found().header("Location", "/admin/tags/").finish()
}

#[get("/admin/reload/")]
async fn reload(data: web::Data<AppState>) -> impl Responder {
    for entry in WalkDir::new(&data.root_dir).into_iter().filter_map(|e| e.ok()) {
        let file_path = entry.path().to_str().unwrap();
        if entry.path().starts_with(Path::new(&data.thumbnail_dir)) {
            continue;
        }
        let root_path = Path::new(&data.root_dir);
        // let rel_path = file_path.replacen(&data.root_dir, "", 1);
        let rel_path = entry.path().strip_prefix(root_path).unwrap().to_str().unwrap_or("");
        if rel_path == "" {
            continue;
        }
        let file_name = entry.file_name().to_str().unwrap();
        let mut file_type = if entry.path().is_dir() { "folder" } else { guess_file_type(file_name) };

        if file_type == "video" {
            if entry.metadata().unwrap().len() < 5242880 {
                file_type = "video/short";
            }
        }

        let mut item = match find_item(&data.conn, &format!("path = \"{}\"", rel_path)) {
            Ok(_item) => _item,
            Err(_) => Item::new(file_name.to_owned(), rel_path.to_string(), file_type.to_owned()),
        };

        let mut children: Vec<String> = Vec::new();
        if file_type == "folder" {
            for i in read_dir(file_path).unwrap() {
                // let child_path = i.unwrap().path().strip_prefix(data.root_dir.as_path()).unwrap().join(data.thumbnail_dir.as_path()).to_str().unwrap().to_string();
                let rel_path = i.unwrap().path();
                let rel_path = rel_path.strip_prefix(&data.root_dir).unwrap();
                let child_path = data.thumbnail_dir.join(rel_path);
                let child_path = child_path.to_str().unwrap();
                children.push(format!("{}.jpg", child_path));
                if children.len() >= 4 {
                    break;
                }
            }
        }

        create_thumbnail(data.root_dir.to_str().unwrap(), data.thumbnail_dir.to_str().unwrap(), file_path, file_type, children);

        let parent_folder = Path::new(&item.path).parent().unwrap_or(Path::new("")).to_str().unwrap();
        if !parent_folder.is_empty() {
            if let Ok(_item) = find_item(&data.conn, &format!("path=\"{}\"", parent_folder)) {
                item.parent = _item.id;
            } else {
                item.parent = 0;
            }
        }

        if item.id == 0 && file_type != "unknown" {
            let mut md5 = Md5::new();
            if file_type == "folder" {
                md5.update(item.path.as_str());
            } else {
                if let Ok(mut file) = File::open(&file_path) {
                    io::copy(&mut file, &mut md5);
                }
            }
            item.md5 = format!("{:x}", md5.finalize());
            match find_item(&data.conn, &format!("md5 = \"{}\"", item.md5)) {
                Ok(_) => println!("{}: duplicated md5sum {}.", item.path, item.md5),
                Err(_) => {
                    if let Err(err) = insert_item(&data.conn, &item).await {
                        eprintln!("Failed to insert item. {}", err);
                    }
                }
            };
        } else {
            if item.file_type != file_type {
                data.conn.execute("UPDATE item SET file_type = ?1 WHERE id = ?2",
                                  params![file_type, item.id],
                ).expect("Update item successfully");
            }
        }
    }

    HttpResponse::Found().header("Location", "/admin/").finish()
}

#[get("/upload/")]
async fn upload(data: web::Data<AppState>, tmpl: web::Data<tera::Tera>, query: web::Query<QueryInfo>) -> impl Responder {
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

    let folders = find_items(&data.conn, "SELECT * FROM item WHERE file_type = \"folder\"").unwrap_or(vec!());
    ctx.insert("parents", &folders);

    let mut all_tags: Vec<Tag> = Vec::new();
    if let Ok(mut tags) = find_tags(&data.conn, None).await {
        tags.sort_by(|a, b| a.name.cmp(&b.name));
        all_tags = tags;
    }
    ctx.insert("tags", &all_tags);

    let template = tmpl.render("upload.html", &ctx).map_err(|_| error::ErrorInternalServerError("Template error")).unwrap();
    HttpResponse::Ok().content_type("text/html").body(template)
}

#[post("/upload/")]
async fn upload_item(data: web::Data<AppState>, mut payload: Multipart) -> impl Responder {
    let tmp_dir_path = Path::new(&data.root_dir).join("tmp");
    if !tmp_dir_path.exists() {
        if let Err(_) = create_dir_all(&tmp_dir_path) {
            HttpResponse::Found().header("Location", "/").finish();
        };
    }

    // TODO: Upload multiple files
    let mut file_name = String::new();
    let mut real_file_name = String::new();

    while let Ok(Some(mut field)) = payload.try_next().await {
        let mut md5_context = Md5::new();
        let mut md5sum = String::new();

        let content_type = field
            .content_disposition()
            .ok_or_else(|| actix_web::error::ParseError::Incomplete).unwrap();
        file_name = content_type
            .get_filename()
            .ok_or_else(|| actix_web::error::ParseError::Incomplete).unwrap().to_string();
        real_file_name = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros().to_string();
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
        let mut f = async_std::fs::File::create(file_path.to_str().unwrap()).await.unwrap();

        // Field in turn is stream of *Bytes* object
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            md5_context.update(&data);
            f.write_all(&data).await;
        };

        md5sum = format!("{:x}", md5_context.finalize());

        let new_file_name = format!("{}.{}", md5sum, ext);
        let new_file_path = tmp_dir_path.join(&new_file_name);
        if let Ok(_) = rename(&file_path, &new_file_path) {
            return HttpResponse::Found().header("Location", format!("/upload/?file_name={}&real_file_name={}&md5={}", file_name, new_file_name, md5sum)).finish();
        }
    }

    HttpResponse::Found().header("Location", "/").finish()
}

#[post("/post_upload/")]
async fn post_upload(data: web::Data<AppState>, form: web::Form<PostData>) -> impl Responder {
    let mut dest_dir = data.root_dir.clone();
    let mut item = Item::empty();
    if let Some(parent) = &form.parent {
        if let Ok(parent_item) = find_item(&data.conn, &format!("id={} AND file_type=\"folder\"", parent)) {
            dest_dir = data.root_dir.join(Path::new(&parent_item.path));
            item.parent = parent_item.id;
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
            item.path = dest_file.strip_prefix(data.root_dir.as_path()).unwrap().to_str().unwrap().to_string();
            item.file_type = guess_file_type(real_file_name).to_string();
            item.md5 = form.md5.as_ref().unwrap().clone();
            if let Ok(id) = insert_item(&data.conn, &item).await {
                if let Some(_tags) = &form.tags {
                    let tags: Vec<&str> = _tags.split_whitespace().collect();
                    if let Err(err) = update_item_tags(&data.conn, id, tags).await {
                        eprintln!("Failed to update tag. {}", err);
                    }
                }

                create_thumbnail(data.root_dir.to_str().unwrap(), data.thumbnail_dir.to_str().unwrap(),
                                 dest_file.to_str().unwrap(), &item.file_type, Vec::new());
                return HttpResponse::Found().header("Location", format!("/?id={}", id)).finish();
            }
        }
    }

    HttpResponse::Found().header("Location", "/upload/").finish()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let mut config = Ini::new();
    let _ = config.load("config.ini");
    let config_root_dir = config.get("default", "root").unwrap();
    let root_dir = Path::new(&config_root_dir).to_path_buf();
    let thumbnail_dir = root_dir.join("thumbnail");
    let db_path = config.get("default", "db").unwrap();
    let port = config.get("default", "port").unwrap();
    let ipp: u64 = config.get("default", "ipp").unwrap_or("48".to_owned()).parse().unwrap();

    HttpServer::new(move || {
        let tera = Tera::new(concat!(env!("CARGO_MANIFEST_DIR"), "/res/html/**/*")).unwrap();

        App::new()
            .data(tera)
            .data(AppState {
                conn: Connection::open(&db_path).unwrap(),
                ipp,
                root_dir: root_dir.clone(),
                thumbnail_dir: thumbnail_dir.clone(),
            })
            .service(index)
            .service(admin)
            .service(manage_tags)
            .service(manage_tag)
            .service(tag_update)
            .service(tag_delete)
            .service(reload)
            .service(item_update)
            .service(delete)
            .service(upload)
            .service(upload_item)
            .service(post_upload)
            .service(Files::new("/img", root_dir.clone()))
            .service(Files::new("/css", concat!(env!("CARGO_MANIFEST_DIR"), "/res/css")))
            .service(Files::new("/js", concat!(env!("CARGO_MANIFEST_DIR"), "/res/js")))
    })
        .bind(format!("0.0.0.0:{}", port))?
        .run()
        .await
}
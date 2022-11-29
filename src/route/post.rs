use actix_web::{get, post, web, HttpResponse, Responder};
use serde::Deserialize;
use std::fs::{create_dir_all, rename};
use std::path::{Path, PathBuf};

use super::{redirect, AppState};
use crate::db::{item, tag};

#[derive(Deserialize)]
pub struct PostData {
    id: Option<i64>,
    pub(crate) tags: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) real_name: Option<String>,
    pub(crate) parent: Option<String>,
    pub(crate) md5: Option<String>,
}

#[post("/")]
pub async fn item_update(
    data: web::Data<AppState>,
    postdata: web::Form<PostData>,
) -> impl Responder {
    if let Some(id) = postdata.id {
        if let Ok(mut item) = item::find_by_id(&data.pool, id).await {
            if let Some(_tags) = &postdata.tags {
                let tags: Vec<&str> = _tags.split_whitespace().collect();
                tag::update_item_tags(&data.pool, id, tags).await;
            }

            let name = postdata.name.as_ref().unwrap();
            let parent = postdata.parent.as_ref().unwrap();
            if let Ok(parent_id) = parent.parse::<i64>() {
                if let Ok(new_parent) = item::find_by_id(&data.pool, parent_id).await {
                    let new_parent_path = PathBuf::from(&new_parent.path);
                    let item_path = Path::new(&item.path);
                    let src_file = data.root_dir.join(item_path);
                    let mut dest_file = PathBuf::new();
                    if let Some(parent_id) = item.parent {
                        if let Ok(old_parent) = item::find_by_id(&data.pool, parent_id).await {
                            let old_parent_path = PathBuf::from(&old_parent.path);
                            dest_file =
                                src_file.strip_prefix(&data.root_dir).unwrap().to_path_buf();
                            dest_file = dest_file
                                .strip_prefix(old_parent_path)
                                .unwrap()
                                .to_path_buf();
                        }
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
                            new_path = dest_file
                                .strip_prefix(&data.root_dir)
                                .unwrap()
                                .to_path_buf();
                            let src_thumb = format!(
                                "{}/{}.jpg",
                                data.thumbnail_dir.to_str().unwrap(),
                                item.path
                            );
                            let dest_thumb = format!(
                                "{}/{}.jpg",
                                data.thumbnail_dir.to_str().unwrap(),
                                new_path.to_str().unwrap()
                            );
                            let thumb_parent_path = format!(
                                "{}/{}",
                                data.thumbnail_dir.to_str().unwrap(),
                                new_parent.path
                            );
                            let thumb_parent = Path::new(&thumb_parent_path);
                            if !thumb_parent.exists() {
                                create_dir_all(&thumb_parent);
                            }

                            rename(src_thumb, dest_thumb);
                        }
                    }
                    item.name = name.to_string();
                    item.parent = Some(parent_id);
                    item.path = new_path.to_str().unwrap_or("").to_string();
                    item::update(&data.pool, item).await;
                }
            }
        }
        return redirect!(format!("/?id={}", id));
    }

    redirect!("/")
}

#[get("/delete/{id}")]
pub async fn delete(data: web::Data<AppState>, id: web::Path<i64>) -> impl Responder {
    item::delete_item(&data.pool, id.into_inner(), data.root_dir.to_str().unwrap()).await;
    redirect!("/")
}

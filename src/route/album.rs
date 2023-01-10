use actix_web::{post, web, get, HttpResponse, Responder, error};
use serde::Deserialize;

use super::{redirect, AppState};
use crate::db::item;

#[derive(Deserialize)]
pub struct FolderForm {
    name: Option<String>,
    parent: Option<String>,
}


#[get("/album/new/")]
pub async fn get_new(
    tmpl: web::Data<tera::Tera>,
    data: web::Data<AppState>,
) -> impl Responder {
    // context to pass data to html template
    let mut ctx = tera::Context::new();

    // List of folders
    let folders = item::find_by_type(&data.pool, "folder")
        .await
        .unwrap_or(vec![]);
    ctx.insert("folders", &folders);

    // Parent of showing item
    let mut parent = 0;


    let template = tmpl
        .render("album.html", &ctx)
        .map_err(|_| error::ErrorInternalServerError("Template error"))
        .unwrap();
    HttpResponse::Ok().content_type("text/html").body(template)
}


#[post("/album/new/")]
pub async fn post_new(
    data: web::Data<AppState>,
    folderform: web::Form<FolderForm>,
) -> impl Responder {
    let name = folderform.name.as_ref().unwrap();
    let parent = folderform.parent.as_ref().unwrap();

    redirect!(format!("/admin/tag/{}", name))
}

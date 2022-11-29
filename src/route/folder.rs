use actix_web::{post, web, HttpResponse, Responder};
use serde::Deserialize;

use super::{redirect, AppState};

#[derive(Deserialize)]
pub struct FolderForm {
    name: Option<String>,
    parent: Option<String>,
}

#[post("/folder/new")]
pub async fn new_folder(
    data: web::Data<AppState>,
    folderform: web::Form<FolderForm>,
) -> impl Responder {
    let name = folderform.name.as_ref().unwrap();
    let parent = folderform.parent.as_ref().unwrap();

    redirect!(format!("/admin/tag/{}", name))
}

use actix_web::{error, get, web, HttpResponse, Responder};
use serde::Serialize;

use super::{AppState, QueryInfo};
use crate::db::{item, tag};

#[derive(Serialize)]
struct Pages {
    cur: u32,
    total: i64,
}

#[get("/")]
pub async fn index(
    tmpl: web::Data<tera::Tera>,
    data: web::Data<AppState>,
    query: web::Query<QueryInfo>,
) -> impl Responder {
    // context to pass data to html template
    let mut ctx = tera::Context::new();
    ctx.insert("listview", &false);

    // query to pass to next URL
    let mut old_query = Vec::new();

    // Tags of showing items to display in side bar
    let mut page_tags;

    // All tags and its count
    let all_tags = tag::count_tags(&data.pool).await.unwrap_or_default();
    ctx.insert("tags", &all_tags);

    // Show original item instead of thumbnail
    let raw = query.raw.unwrap_or_default();
    ctx.insert("raw", &raw);
    old_query.push(format!("raw={}", raw));

    // Items to show
    let mut items = Vec::new();

    // Current page
    let page = query.page.unwrap_or(1);

    // View mode
    let view = query.view.as_deref().unwrap_or_default();
    old_query.push(format!("view={}", view));
    ctx.insert("view", &view);

    // List of folders
    let folders = item::find_by_type(&data.pool, "folder")
        .await
        .unwrap_or(vec![]);
    ctx.insert("folders", &folders);

    // Offset for LIMIT clause
    let offset = (page as i64 - 1) * data.ipp;

    // Total number of items
    let mut count = 0;

    // Parent of showing item
    let mut parent = 0;

    let id = query.id.unwrap_or_default();
    if id > 0 {
        old_query.push(format!("id={}", id));
        match item::find_by_id(&data.pool, id).await {
            Ok(item) => {
                parent = item.parent.unwrap_or_default();
                page_tags = tag::find_by_items(&data.pool, vec![id])
                    .await
                    .unwrap_or_default();
                ctx.insert("item", &item);
                ctx.insert("page_tags", &page_tags);

                let listview = page_tags
                    .iter()
                    .map(|t| t.name.clone())
                    .collect::<Vec<String>>()
                    .contains(&"series".to_string());
                ctx.insert("listview", &listview);

                if item.file_type == "folder" {
                    (items, count) =
                        item::find_by_parent(&data.pool, Some(id), Some(data.ipp), Some(offset))
                            .await
                            .unwrap_or_default();
                } else {
                    ctx.insert("parent", &parent);
                    let template = tmpl
                        .render("post.html", &ctx)
                        .map_err(|_| error::ErrorInternalServerError("Template error"))
                        .unwrap();
                    return HttpResponse::Ok().content_type("text/html").body(template);
                }
            }
            Err(err) => {
                println!("Cannot find item: {:?}", err);
                return HttpResponse::Ok().body("Not found!");
            }
        }
    } else {
        // tags that will be searched for
        let searching_tags_str = query.tags.as_deref().unwrap_or_default();
        let searching_tags: Vec<String> = searching_tags_str
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();

        if searching_tags.len() > 0 {
            old_query.push(format!("tags={}", searching_tags_str));
            (items, count) = item::find_by_tag(&data.pool, searching_tags, data.ipp, offset)
                .await
                .unwrap_or_default();
        } else {
            // Find all items that not in a series
            (items, count) = item::find_not_in_series(&data.pool, data.ipp, offset)
                .await
                .unwrap_or_default();
        }

        let item_ids: Vec<i64> = items.iter().map(|i| i.id).collect();
        page_tags = tag::find_by_items(&data.pool, item_ids)
            .await
            .unwrap_or_default();
    }

    let total_page = count / data.ipp + if count % data.ipp != 0 { 1 } else { 0 };
    let pages = Pages {
        cur: page,
        total: total_page,
    };
    ctx.insert("pages", &pages);

    ctx.insert("items", &items);
    ctx.insert("old_query", &old_query.join("&"));
    ctx.insert("item_id", &id);
    ctx.insert("parent", &parent); // TODO: In template, get parent from item instead
    ctx.insert("page_tags", &page_tags);

    let template = tmpl
        .render("index.html", &ctx)
        .map_err(|_| error::ErrorInternalServerError("Template error"))
        .unwrap();
    HttpResponse::Ok().content_type("text/html").body(template)
}

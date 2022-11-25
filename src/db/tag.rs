use super::{item_tag, tag_tag};

use std::collections::HashMap;

use async_recursion::async_recursion;
use serde::Serialize;
use sqlx::sqlite::SqliteQueryResult;
use sqlx::{Error, SqlitePool};

#[derive(Serialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub alias: Option<i64>,
    pub created_at: String,
}

macro_rules! insert {
    ($pool: expr, $name: expr) => {
        sqlx::query!(r#"INSERT INTO tag (name) VALUES (?)"#, $name)
            .execute($pool)
            .await?
            .last_insert_rowid()
    };
}

macro_rules! find_one_by_column {
    ($col: literal, $val: expr, $pool: expr) => {
        sqlx::query_as!(Tag, "SELECT * FROM tag WHERE " + $col + " = ?", $val)
            .fetch_one($pool)
            .await
    };
}

macro_rules! delete_by_column {
    ($pool: expr, $col: literal, $val: expr) => {
        sqlx::query!("DELETE FROM tag WHERE " + $col + " = ?", $val)
            .execute($pool)
            .await
    };
}

pub async fn find_all(pool: &SqlitePool) -> Result<Vec<Tag>, sqlx::Error> {
    sqlx::query_as!(Tag, r#"SELECT * FROM tag ORDER BY name ASC"#)
        .fetch_all(pool)
        .await
}

pub async fn find_by_id(pool: &SqlitePool, id: i64) -> Result<Tag, sqlx::Error> {
    find_one_by_column!("id", id, pool)
}

pub async fn find_by_name(pool: &SqlitePool, name: &str) -> Result<Tag, sqlx::Error> {
    find_one_by_column!("name", name, pool)
}

pub async fn find_by_items(pool: &SqlitePool, item_ids: Vec<i64>) -> Result<Vec<Tag>, sqlx::Error> {
    let mut ids = Vec::new();
    for id in item_ids {
        ids.push(id.to_string());
    }

    // TODO: update when sqlx support carray https://github.com/launchbadge/sqlx/issues/1113
    let ids_join = format!("[{}]", ids.join(","));
    sqlx::query_as!(Tag, r#"SELECT tag.id, name, alias, created_at from tag
        LEFT JOIN item_tag ON tag.id = item_tag.tag
        WHERE item_tag.item IN (SELECT value FROM JSON_EACH(?)) GROUP BY tag.id ORDER BY tag.name ASC"#, ids_join).fetch_all(pool).await
}

pub async fn find_or_create(pool: &SqlitePool, name: &str) -> Result<Tag, sqlx::Error> {
    let name = name.to_lowercase();
    if let Ok(tag) = find_by_name(pool, &name).await {
        Ok(tag)
    } else {
        let id = insert!(pool, name);
        find_by_id(pool, id).await
    }
}

pub async fn find_depend_tags(pool: &SqlitePool, id: i64) -> Result<Vec<Tag>, sqlx::Error> {
    sqlx::query_as!(Tag, r#"SELECT tag.id, tag.name, tag.created_at, tag.alias FROM tag LEFT JOIN tag_tag ON tag.id = tag_tag.dep WHERE tag_tag.tag=?"#, id).fetch_all(pool).await
}

async fn delete_by_id(pool: &SqlitePool, id: i64) -> Result<SqliteQueryResult, sqlx::Error> {
    delete_by_column!(pool, "id", id)
}

async fn update_name(
    pool: &SqlitePool,
    name: &str,
    id: i64,
) -> Result<SqliteQueryResult, sqlx::Error> {
    sqlx::query!(r#"UPDATE tag SET name=? WHERE id = ?"#, name, id)
        .execute(pool)
        .await
}

pub async fn update_item_tags(
    pool: &SqlitePool,
    item_id: i64,
    tag_names: Vec<&str>,
) -> Result<(), sqlx::Error> {
    let mut tags = Vec::new();
    for tag_name in tag_names {
        if let Ok(tag) = find_or_create(pool, &tag_name.to_lowercase()).await {
            tags.push(tag.id);
        }
    }
    item_tag::delete_by_item(pool, item_id).await?;
    item_tag::insert_many(pool, item_id, tags).await?;

    Ok(())
}

pub async fn update_tag(pool: &SqlitePool, id: i64, name: &str, deps: Vec<&str>) {
    let mut dep_ids = Vec::new();
    for dep in deps {
        if dep == name {
            continue;
        }
        if let Ok(tag) = find_or_create(pool, dep).await {
            dep_ids.push(tag.id);
        }
    }

    let mut old_deps = Vec::new();
    let dep_tags = tag_tag::find_by_tag(pool, id).await.unwrap_or(vec![]);
    for d in dep_tags {
        if !dep_ids.contains(&d.dep) {
            tag_tag::delete_by_id(pool, d.id).await;
        } else {
            old_deps.push(d.dep);
        }
    }

    update_name(pool, name, id).await;

    for dep_id in dep_ids {
        if !old_deps.contains(&dep_id) {
            tag_tag::insert(pool, id, dep_id).await;
        }
    }
}

pub async fn delete_tag(pool: &SqlitePool, id: i64) {
    item_tag::delete_by_tag(pool, id).await;
    tag_tag::delete_relate_tag(pool, id).await;
    delete_by_id(pool, id).await;
}

pub async fn count_tags(pool: &SqlitePool) -> Result<HashMap<String, i32>, sqlx::Error> {
    let mut ret: HashMap<String, i32> = HashMap::new();
    let recs = sqlx::query!(
        r#"SELECT tag.name as name, COUNT(item_tag.tag) as count
    FROM tag LEFT JOIN item_tag ON tag.id = item_tag.tag GROUP BY tag.name ORDER BY count DESC"#
    )
    .fetch_all(pool)
    .await?;
    for rec in recs {
        ret.insert(rec.name, rec.count.unwrap_or(0));
    }
    Ok(ret)
}

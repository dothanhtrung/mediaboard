use std::fs::{remove_dir_all, remove_file};
use std::path::Path;

use async_recursion::async_recursion;
use serde::Serialize;
use sqlx::sqlite::SqliteQueryResult;
use sqlx::SqlitePool;
use crate::{item_tag, tag};

#[derive(Serialize)]
pub struct Item {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub file_type: String,
    pub created_at: String,
    pub parent: Option<i64>,
    pub md5: String,
}

macro_rules! insert {
    ($name: expr, $path: expr, $file_type: expr, $md5: expr, $pool: expr) => {
        sqlx::query!(r#"INSERT INTO item (name, path, file_type, md5) VALUES (?, ?, ?, ?)"#,
            $name,
            $path,
            $file_type,
            $md5).execute($pool).await?.last_insert_rowid()
    };
    ($name: expr, $path: expr, $file_type: expr, $md5: expr, $parent: expr, $pool: expr) => {
        sqlx::query!(r#"INSERT INTO item (name, path, file_type, parent, md5) VALUES (?, ?, ?, ?, ?)"#,
            $name,
            $path,
            $file_type,
            $parent,
            $md5).execute($pool).await?.last_insert_rowid()
    }
}

macro_rules! update {
    ($id: expr, $name: expr, $path: expr, $parent: expr, $pool: expr) => {
        sqlx::query!(r#"UPDATE item SET name=?, parent=?, path=? WHERE id = ?"#,
            $name,
            $parent,
            $path,
            $id).execute($pool).await
    }
}

macro_rules! find_by_column {
    ($pool: expr, $col: literal, $val: expr) => {
        sqlx::query_as!(Item, "SELECT * FROM item WHERE " + $col + " = ?", $val).fetch_all($pool).await
    };
    ($pool: expr, $col: literal, $val: expr, $limit: expr, $offset: expr, $order: literal) => {
        sqlx::query_as!(Item, r#"SELECT item.id as "id!", item.name as "name!", item.path as "path!",
                                    item.file_type as "file_type!", item.created_at as "created_at!",
                                    item.parent as parent, item.md5 as "md5!"
                        FROM item WHERE "# + $col + " = ? ORDER BY " + $order + " LIMIT ? OFFSET ?"
        , $val, $limit, $offset).fetch_all($pool).await
    }
}

macro_rules! find_one_by_column {
    ($col: expr, $val: expr, $pool: expr) => {
        sqlx::query_as!(Item, "SELECT * FROM item WHERE " + $col + " = ?", $val).fetch_one($pool).await
    };
    ($col1: expr, $val1: expr, $col2: expr, $val2: expr, $pool: expr) => {
        sqlx::query_as!(Item, "SELECT * FROM item WHERE " + $col1 + " = ? AND " + $col2 + " = ?", $val1, $val2).fetch_one($pool).await
    }
}

// TODO: Shorten code by macro
// macro_rules! find_and_count {
//     ($pool: expr, $query: literal, $start: expr, $end: expr, $($arg:expr),*) => {
//         let items = sqlx::query_as!(Item,
//             r#"SELECT item.id as "id!", item.name as "name!", item.path as "path!",
//                 item.file_type as "file_type!", item.created_at as "created_at!",
//                 item.parent as parent, item.md5 as "md5!""#
//                 + $query +
//                 r#"ORDER BY item.created_at DESC LIMIT ?, ?"#
//             , $($arg,)* , start, end).fetch_all(pool).await?;
//         let count = sqlx::query!(r#"SELECT COUNT(*) as count"# + $query
//             $(, $arg)*).fetch_one(pool).await?;
//
//         return Ok((items, count.count));
//     };
// }

macro_rules! delete_by_column {
    ($col: expr, $val: expr, $pool: expr) => {
        sqlx::query!("DELETE FROM item WHERE " + $col + " = ?", $val).execute($pool).await
    }
}

impl Item {
    pub fn empty() -> Item {
        Item {
            id: 0,
            name: String::new(),
            path: String::new(),
            file_type: String::new(),
            created_at: String::new(),
            parent: None,
            md5: String::new(),
        }
    }

    pub fn new(name: String, path: String, file_type: String) -> Item {
        Item {
            id: 0,
            name,
            path,
            file_type,
            created_at: String::new(),
            parent: None,
            md5: String::new(),
        }
    }
}

pub async fn insert(pool: &SqlitePool, item: &Item) -> Result<i64, sqlx::Error> {
    if item.parent != None {
        let id = insert!(item.name, item.path, item.file_type, item.md5, item.parent, pool);
        Ok(id)
    } else {
        let id = insert!(item.name, item.path, item.file_type, item.md5, pool);
        Ok(id)
    }
}

pub async fn update(pool: &SqlitePool, item: Item) -> Result<SqliteQueryResult, sqlx::Error> {
    update!(item.id, item.name, item.path, item.parent, pool)
}

pub async fn find_by_type(pool: &SqlitePool, file_type: &str) -> Result<Vec<Item>, sqlx::Error> {
    find_by_column!(pool, "file_type", file_type)
}

pub async fn find_by_id(pool: &SqlitePool, id: i64) -> Result<Item, sqlx::Error> {
    find_one_by_column!("id", id, pool)
}

pub async fn find_by_path(pool: &SqlitePool, path: &str) -> Result<Item, sqlx::Error> {
    find_one_by_column!("path", path, pool)
}

pub async fn find_by_md5(pool: &SqlitePool, md5: &str) -> Result<Item, sqlx::Error> {
    find_one_by_column!("md5", md5, pool)
}

pub async fn find_by_parent(pool: &SqlitePool, parent: Option<i64>, limit: Option<i64>, offset: Option<i64>) -> Result<(Vec<Item>, i64), sqlx::Error> {
    let items;
    let mut is_series = false;
    let tags = tag::find_by_items(pool, vec![parent.unwrap()]).await?;
    for tag in tags {
        if tag.name == "series" {
            is_series = true;
            break;
        }
    }

    if limit == None || offset == None {
        items = find_by_column!(pool, "parent", parent)?;
    } else {
        if is_series {
            items = find_by_column!(pool, "parent", parent, limit, offset, "name ASC")?;
        }
        else {
            items = find_by_column!(pool, "parent", parent, limit, offset, "created_at DESC")?;
        }
    }

    let count = sqlx::query!("SELECT COUNT(*) as count FROM item WHERE parent = ?", parent).fetch_one(pool).await?;
    Ok((items, count.count as i64))
}

pub async fn find_by_tag(pool: &SqlitePool, tags: Vec<String>, limit: i64, offset: i64) -> Result<(Vec<Item>, i64), sqlx::Error> {
    let tags = tags.join(",");
    let items = sqlx::query_as!(Item, r#"SELECT item.id as "id!", item.name as "name!", item.path as "path!",
                                    item.file_type as "file_type!", item.created_at as "created_at!",
                                    item.parent as parent, item.md5 as "md5!"
    FROM item LEFT JOIN item_tag ON item_tag.item = item.id
              LEFT JOIN tag ON item_tag.tag = tag.id
    WHERE tag.name IN (?)
    GROUP BY item.id
    ORDER BY item.created_at DESC LIMIT ? OFFSET ?"#
        ,tags ,limit, offset).fetch_all(pool).await?;
    let count = sqlx::query!(r#"SELECT COUNT(DISTINCT item.id) as count
    FROM item LEFT JOIN item_tag ON item_tag.item = item.id
              LEFT JOIN tag ON item_tag.tag = tag.id
    WHERE tag.name IN (?)"#
        ,tags).fetch_one(pool).await?;

    Ok((items, count.count.into()))
}

pub async fn find_not_in_series(pool: &SqlitePool, limit: i64, offset: i64) -> Result<(Vec<Item>, i64), sqlx::Error> {
    let items = sqlx::query_as!(Item,
            r#"SELECT item.id as "id!", item.name as "name!", item.path as "path!",
                      item.file_type as "file_type!", item.created_at as "created_at!",
                      item.parent as parent, item.md5 as "md5!"
            FROM item WHERE parent NOT IN (
                SELECT item.id FROM item LEFT JOIN item_tag ON item_tag.item = item.id
                LEFT JOIN tag ON item_tag.tag = tag.id
                WHERE tag.name == "series") OR parent is null
            ORDER BY created_at DESC LIMIT ? OFFSET ?"#
        ,limit, offset).fetch_all(pool).await?;

    let count = sqlx::query!(r#"SELECT COUNT(*) as count
            FROM item WHERE parent NOT IN (
                SELECT item.id FROM item LEFT JOIN item_tag ON item_tag.item = item.id
                LEFT JOIN tag ON item_tag.tag = tag.id
                WHERE tag.name == "series"
            ) OR parent is null "#
        ).fetch_one(pool).await?;

    Ok((items, count.count as i64))
}

pub async fn delete_by_id(pool: &SqlitePool, id: i64) {
    delete_by_column!("id", id, pool);
}

pub async fn delete_by_parent(pool: &SqlitePool, parent: Option<i64>) {
    delete_by_column!("parent", parent, pool);
}

pub async fn delete_local_file(file_path: &str) -> Result<(), std::io::Error> {
    let path = Path::new(&file_path);
    if path.is_dir() {
        remove_dir_all(path)
    } else {
        remove_file(path)
    }
}

#[async_recursion]
pub async fn delete_item(pool: &SqlitePool, id: i64, root_dir: &str) {
    // let trash_dir = format!("{}/trash", root_dir);
    // let trash_dir_path = Path::new(&trash_dir);
    // if !trash_dir_path.exists() && !trash_dir_path.is_dir() {
    //     create_dir_all(trash_dir_path);
    // }

    item_tag::delete_by_item(pool, id).await;

    let (items, count) = find_by_parent(pool, Some(id), None, None).await.unwrap_or_default();
    for item in items {
        delete_item(pool, item.id, root_dir).await;
    }
    delete_by_parent(pool, Some(id)).await;

    if let Ok(item) = find_by_id(pool, id).await {
        let file_path = format!("{}/{}", root_dir, item.path);
        let thumbnail_path = format!("{}/thumbnail/{}.jpg", root_dir, item.path);
        delete_local_file(&file_path).await;
        delete_local_file(&thumbnail_path).await;
        delete_by_id(pool, id).await;
    }
}
use rusqlite::{params, Connection, Result};
use serde::Serialize;
use sqlx::SqlitePool;

use std::collections::HashMap;
use std::fs::{remove_dir_all, remove_file};
use std::path::Path;

#[derive(Serialize)]
pub struct Item {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub file_type: String,
    pub created_at: String,
    pub parent: i64,
    pub cover: String,
    pub md5: String,
}

#[derive(Serialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
}

impl Item {
    pub fn empty() -> Item {
        Item {
            id: 0,
            name: String::new(),
            path: String::new(),
            file_type: String::new(),
            created_at: String::new(),
            parent: 0,
            cover: String::new(),
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
            parent: 0,
            cover: String::new(),
            md5: String::new(),
        }
    }
}

pub async fn insert_item(pool: &SqlitePool, item: &Item) -> Result<i64, sqlx::Error> {
    let ret = sqlx::query(r#"INSERT INTO item (name, path, file_type, parent, md5) VALUES (?, ?, ?, ?, ?)"#)
        .bind(item.name.as_str())
        .bind(item.path.as_str())
        .bind(item.file_type.as_str())
        .bind(item.parent)
        .bind(item.md5.as_str())
        .execute(pool).await;
    match ret {
        Ok(res) => Ok(res.last_insert_rowid()),
        Err(e) => Err(e),
    }
}

pub fn find_items(conn: &Connection, query: &str) -> Result<Vec<Item>> {
    let mut stmt = conn.prepare(query)?;
    let mut rows = stmt.query([])?;
    let mut items = Vec::new();
    while let Some(row) = rows.next()? {
        let id = row.get(0)?;
        let mut first_child_path = String::new();
        if let Ok(first_child) = find_item(conn, &format!("parent = {} AND file_type=\"image\"", id)) {
            first_child_path = first_child.path;
        }
        items.push(Item {
            id,
            name: row.get(1)?,
            path: row.get(2)?,
            file_type: row.get(3)?,
            created_at: row.get(4)?,
            parent: row.get(5)?,
            cover: first_child_path,
            md5: row.get(6)?,
        });
    }
    return Ok(items);
}

pub async fn find_tags(conn: &Connection, cond: Option<&str>) -> Result<Vec<Tag>> {
    let mut query = String::from("SELECT * FROM tag");
    if let Some(_cond) = cond {
        query = format!("{} WHERE {}", query, _cond);
    }
    query.push_str(" ORDER BY name ASC");
    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query([])?;
    let mut tags = Vec::new();
    while let Some(row) = rows.next()? {
        tags.push(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
        });
    }
    return Ok(tags);
}

pub fn find_item(conn: &Connection, cond: &str) -> Result<Item> {
    let mut stmt = conn.prepare(&format!("SELECT * FROM item WHERE {}", cond))?;
    stmt.query_row([], |row| {
        Ok(Item {
            id: row.get(0)?,
            name: row.get(1)?,
            path: row.get(2)?,
            file_type: row.get(3)?,
            created_at: row.get(4)?,
            parent: row.get(5)?,
            cover: String::new(),
            md5: row.get(6)?,
        })
    })
}

pub fn find_item_ids(conn: &Connection, query: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(query)?;
    let mut rows = stmt.query([])?;
    let mut ids = Vec::new();
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        ids.push(id.to_string());
    }

    Ok(ids)
}

pub fn count_items(conn: &Connection, sql: &String) -> Result<u64> {
    let mut stmt = conn.prepare(&format!("SELECT COUNT(*) FROM ({})", sql))?;
    stmt.query_row([], |row| {
        Ok(row.get(0)?)
    })
}

pub fn find_tags_by_items(conn: &Connection, item_ids: Vec<i64>) -> Result<Vec<Tag>> {
    let mut tags = Vec::new();
    let mut ids = Vec::new();
    for id in item_ids {
        ids.push(id.to_string());
    }
    let mut stmt = conn.prepare(
        &format!("SELECT * from tag JOIN item_tag ON tag.id = item_tag.tag WHERE item in ({}) GROUP BY tag.id", ids.join(",")))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        tags.push(Tag { id: row.get(0)?, name: row.get(1)? });
    }

    Ok(tags)
}

pub fn find_tag_or_create(conn: &Connection, name: &str) -> Result<Tag, String> {
    if name.is_empty() {
        return Err("Empty tag name".to_owned());
    }
    let mut stmt = conn.prepare("SELECT * FROM tag WHERE name=?1").unwrap();
    match stmt.query_row([name], |row| {
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
        })
    }) {
        Ok(tag) => return Ok(tag),
        Err(_) => match conn.execute("INSERT INTO tag (name) VALUES (?1)", params![name]) {
            Ok(_) => return Ok(Tag { id: conn.last_insert_rowid(), name: name.to_owned() }),
            Err(err) => return Err(err.to_string()),
        },
    };
}

pub fn find_tag_by_id(conn: &Connection, id: i64) -> Result<Tag, String> {
    let mut stmt = conn.prepare("SELECT * FROM tag WHERE id=?1").unwrap();
    match stmt.query_row([id], |row| {
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
        })
    }) {
        Ok(tag) => return Ok(tag),
        Err(err) => return Err(err.to_string()),
    };
}

pub fn find_depend_tags(conn: &Connection, id: i64) -> Result<Vec<Tag>> {
    let mut query = format!("SELECT * FROM tag LEFT JOIN tag_tag ON tag.id = tag_tag.dep WHERE tag_tag.tag={}", id);
    query.push_str(" ORDER BY name ASC");
    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query([])?;
    let mut tags = Vec::new();
    while let Some(row) = rows.next()? {
        tags.push(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
        });
    }
    return Ok(tags);
}

fn update_item_tag(conn: &Connection, item_id: i64, tag_id: i64) -> Result<()> {
    if let Ok(_) = conn.execute("INSERT INTO item_tag (item, tag) VALUES (?1, ?2)", params![item_id, tag_id]) {
        let mut stmt = conn.prepare("SELECT dep FROM tag_tag WHERE tag=?1").unwrap();
        let mut rows = stmt.query([tag_id])?;
        while let Some(row) = rows.next()? {
            let tag_id = row.get(0)?;
            update_item_tag(conn, item_id, tag_id);
        }
    }
    Ok(())
}

pub async fn update_item_tags(conn: &Connection, item_id: i64, tag_names: Vec<&str>) -> Result<()> {
    let mut tags = Vec::new();
    for tag_name in tag_names {
        if let Ok(tag) = find_tag_or_create(conn, &tag_name.to_lowercase()) {
            tags.push(tag.id);
        }
    }

    let mut old_tags = Vec::new();
    let mut stmt = conn.prepare("SELECT * FROM item_tag WHERE item=?1").unwrap();
    let mut rows = stmt.query([item_id])?;
    while let Some(row) = rows.next()? {
        let tag_id = row.get(2)?;
        if !tags.contains(&tag_id) {
            conn.execute("DELETE FROM item_tag WHERE item = ?1 AND tag = ?2", params![item_id, tag_id])?;
        } else {
            old_tags.push(tag_id);
        }
    }

    for tag in tags {
        if !old_tags.contains(&tag) {
            update_item_tag(conn, item_id, tag);
        }
    }

    Ok(())
}

pub async fn update_item(conn: &Connection, id: i64, name: &str, parent: &str, path: &str) {
    if let Err(err) = conn.execute("UPDATE item SET name=?1, parent=?2, path=?3 WHERE id = ?4", params![name, parent, path, id]) {
        eprintln!("Failed to update item. {}", err);
    }
}

async fn delete_local_file(file_path: &str) {
    let path = Path::new(&file_path);
    if path.is_dir() {
        remove_dir_all(path);
    } else {
        remove_file(path);
    }
}

pub async fn delete_item(conn: &Connection, id: i64, root_dir: &str) {
    // let trash_dir = format!("{}/trash", root_dir);
    // let trash_dir_path = Path::new(&trash_dir);
    // if !trash_dir_path.exists() && !trash_dir_path.is_dir() {
    //     create_dir_all(trash_dir_path);
    // }

    conn.execute("DELETE FROM item_tag WHERE item = ?1", params![id]);

    let items = find_items(conn, &format!("SELECT * FROM item WHERE parent = {}", id)).unwrap();
    for item in items {
        delete_item(conn, item.id, root_dir);
    }
    conn.execute("DELETE FROM item WHERE parent = ?1", params![id]);

    if let Ok(item) = find_item(conn, &format!("id = {}", id)) {
        let file_path = format!("{}/{}", root_dir, item.path);
        let thumbnail_path = format!("{}/thumbnail/{}.jpg", root_dir, item.path);
        delete_local_file(&file_path).await;
        delete_local_file(&thumbnail_path).await;
        conn.execute("DELETE FROM item WHERE id = ?1", params![id]);
    }
}

pub async fn delete_tag(conn: &Connection, id: i64) {
    conn.execute("DELETE FROM item_tag WHERE tag = ?1", params![id]);
    conn.execute("DELETE FROM tag_tag WHERE tag = ?1 OR dep = ?1", params![id]);
    conn.execute("DELETE FROM tag WHERE id = ?1", params![id]);
}

pub async fn update_tag(conn: &Connection, id: i64, name: &str, deps: Vec<&str>) -> Result<()> {
    let mut dep_ids = Vec::new();
    for dep in deps {
        if dep == name {
            continue;
        }
        if let Ok(tag) = find_tag_or_create(conn, dep) {
            dep_ids.push(tag.id);
        }
    }

    let mut old_deps = Vec::new();
    let mut stmt = conn.prepare("SELECT id, dep FROM tag_tag WHERE tag=?1").unwrap();
    let mut rows = stmt.query([id])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let dep = row.get(1)?;
        if !dep_ids.contains(&dep) {
            conn.execute("DELETE FROM tag_tag WHERE id=?1", params![id]);
        } else {
            old_deps.push(dep);
        }
    }

    if let Err(err) = conn.execute("UPDATE tag SET name=?1 WHERE id = ?2", params![name, id]) {
        eprintln!("Failed to update tag. {}", err);
    }

    for dep_id in dep_ids {
        if !old_deps.contains(&dep_id) {
            match conn.execute("INSERT INTO tag_tag (tag, dep) VALUES (?1, ?2)", params![id, dep_id]) {
                Ok(_) => (),
                Err(err) => eprintln!("Failed to insert tag dependency. {}", err),
            };
        }
    }

    return Ok(());
}

pub async fn count_tags(conn: &Connection) -> Result<HashMap<String,u64>> {
    let mut ret :HashMap<String, u64> = HashMap::new();
    let mut stmt = conn.prepare("SELECT tag.name, COUNT(item_tag.tag) FROM tag LEFT JOIN item_tag ON tag.id = item_tag.tag GROUP BY item_tag.tag;")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let tag :String = row.get(0)?;
        let count :u64 = row.get(1)?;
        ret.insert(tag, count);
    }
    Ok(ret)
}
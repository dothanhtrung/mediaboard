use futures::TryStreamExt;
use sqlx::sqlite::SqliteQueryResult;
use sqlx::{Row, SqlitePool};

macro_rules! delete_by_column {
    ($pool: expr, $col: expr, $val: expr) => {
        sqlx::query!("DELETE FROM item_tag WHERE " + $col + " = ?", $val)
            .execute($pool)
            .await
    };
}

pub struct ItemTag {
    pub id: i64,
    pub item: i64,
    pub tag: i64,
}

pub async fn insert(pool: &SqlitePool, item: i64, tag: i64) -> Result<i64, sqlx::Error> {
    let id = sqlx::query!(
        r#"INSERT INTO item_tag (item, tag) VALUES (?, ?)"#,
        item,
        tag
    )
    .execute(pool)
    .await?
    .last_insert_rowid();
    Ok(id)
}

pub async fn insert_many(
    pool: &SqlitePool,
    item: i64,
    tags: Vec<i64>,
) -> Result<SqliteQueryResult, sqlx::Error> {
    let mut values = Vec::new();
    for tag in &tags {
        values.push(format!("({}, {})", item, tag));
    }
    sqlx::query(&format!(
        "INSERT INTO item_tag (item, tag) VALUES {}",
        values.join(",")
    ))
    .execute(pool)
    .await?;

    let mut deps = find_missing_dep_tags(pool, item, tags).await?;
    while deps.len() > 0 {
        values = Vec::new();
        for dep in &deps {
            values.push(format!("({}, {})", item, dep));
        }
        sqlx::query(&format!(
            "INSERT INTO item_tag (item, tag) VALUES {}",
            values.join(",")
        ))
        .execute(pool)
        .await?;
        deps = find_missing_dep_tags(pool, item, deps).await?;
    }
    Ok(Default::default())
}

pub async fn find_missing_dep_tags(
    pool: &SqlitePool,
    item: i64,
    tags: Vec<i64>,
) -> Result<Vec<i64>, sqlx::Error> {
    let stags: Vec<String> = tags.iter().map(|&id| id.to_string()).collect();
    let stags = stags.join(",");

    let query = &format!(
        "SELECT tag_tag.dep as dep FROM tag_tag
    LEFT JOIN item_tag ON tag_tag.dep = item_tag.tag
    WHERE tag_tag.tag IN ({}) AND (item_tag.item IS NULL OR item_tag.item != ?)",
        stags
    );
    let mut rows = sqlx::query(query).bind(item).fetch(pool);

    let mut deps: Vec<i64> = Vec::new();
    while let Some(row) = rows.try_next().await? {
        let dep: i64 = row.try_get(0)?;
        deps.push(dep);
    }
    Ok(deps)
}

pub async fn find_by_item(pool: &SqlitePool, item: i64) -> Result<Vec<ItemTag>, sqlx::Error> {
    sqlx::query_as!(ItemTag, r#"SELECT * FROM item_tag WHERE item = ?"#, item)
        .fetch_all(pool)
        .await
}

pub async fn delete_by_id(pool: &SqlitePool, id: i64) -> Result<SqliteQueryResult, sqlx::Error> {
    delete_by_column!(pool, "id", id)
}

pub async fn delete_by_item(
    pool: &SqlitePool,
    item: i64,
) -> Result<SqliteQueryResult, sqlx::Error> {
    delete_by_column!(pool, "item", item)
}

pub async fn delete_by_tag(pool: &SqlitePool, tag: i64) -> Result<SqliteQueryResult, sqlx::Error> {
    delete_by_column!(pool, "tag", tag)
}

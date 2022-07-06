use sqlx::sqlite::SqliteQueryResult;
use sqlx::SqlitePool;

macro_rules! delete_by_column {
    ($pool: expr, $col: expr, $val: expr) => {
        sqlx::query!("DELETE FROM item_tag WHERE " + $col + " = ?", $val).execute($pool).await
    }
}

pub struct ItemTag {
    pub id: i64,
    pub item: i64,
    pub tag: i64,
}

pub async fn insert(pool: &SqlitePool, item: i64, tag: i64) -> Result<i64, sqlx::Error> {
    let id = sqlx::query!(r#"INSERT INTO item_tag (item, tag) VALUES (?, ?)"#, item, tag).execute(pool).await?.last_insert_rowid();
    Ok(id)
}

pub async fn find_by_item(pool: &SqlitePool, item: i64) -> Result<Vec<ItemTag>, sqlx::Error> {
    sqlx::query_as!(ItemTag, r#"SELECT * FROM item_tag WHERE item = ?"#, item).fetch_all(pool).await
}

pub async fn delete_by_id(pool: &SqlitePool, id: i64) -> Result<SqliteQueryResult, sqlx::Error> {
    delete_by_column!(pool, "id", id)
}

pub async fn delete_by_item(pool: &SqlitePool, item: i64) -> Result<SqliteQueryResult, sqlx::Error> {
    delete_by_column!(pool, "item", item)
}

pub async fn delete_by_tag(pool: &SqlitePool, tag: i64) -> Result<SqliteQueryResult, sqlx::Error> {
    delete_by_column!(pool, "tag", tag)
}
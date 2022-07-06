use sqlx::SqlitePool;

pub struct TagTag {
    pub id: i64,
    pub tag: i64,
    pub dep: i64
}

macro_rules! find_by_column {
    ($pool: expr, $col: expr, $val: expr) => {
        sqlx::query_as!(TagTag, "SELECT * FROM tag_tag WHERE " + $col + " = ?", $val).fetch_all($pool).await
    }
}

macro_rules! delete_by_column {
    ($pool: expr, $col: expr, $val: expr) => {
        sqlx::query!("DELETE FROM tag_tag WHERE " + $col + " = ?", $val).execute($pool).await
    };
    ($pool: expr, $col1: expr, $val1: expr, $col2: expr, $val2: expr) => {
        sqlx::query!("DELETE FROM tag_tag WHERE " + $col1 + " = ? or " + $col2 + " = ?", $val1, $val2).execute($pool).await
    }
}

pub async fn insert(pool: &SqlitePool, tag: i64, dep: i64) -> Result<i64, sqlx::Error> {
    let id = sqlx::query!(r#"INSERT INTO tag_tag (tag, dep) VALUES (?, ?)"#,
            tag, dep).execute(pool).await?.last_insert_rowid();
    Ok(id)
}

pub async fn find_by_tag(pool: &SqlitePool, tag: i64) -> Result<Vec<TagTag>, sqlx::Error> {
    find_by_column!(pool, "tag", tag)
}

pub async fn delete_by_id(pool: &SqlitePool, id: i64) {
    delete_by_column!(pool, "id", id);
}

pub async fn delete_relate_tag(pool: &SqlitePool, tag: i64) {
    delete_by_column!(pool, "tag", tag, "dep", tag);
}
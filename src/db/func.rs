macro_rules! find_by_column {
    ($pool: expr, $tab: literal, $col: literal, $val: expr) => {
        sqlx::query_as!(Item, "SELECT * FROM " + $tab + " WHERE " + $col + " = ?", $val).fetch_all($pool).await
    }
}
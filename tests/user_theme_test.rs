use ahlt::models::user;
use sqlx::PgPool;

#[sqlx::test]
async fn test_get_user_theme_returns_saved_preference(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    // Setup: Create a user entity first
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO entities (entity_type, name, label) VALUES ('user', 'testuser', 'Test User') RETURNING id"
    )
    .fetch_one(&pool)
    .await?;

    let theme = "dark";

    // Set theme via entity_properties
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, $2, $3)"
    )
    .bind(user_id)
    .bind("theme_preference")
    .bind(theme)
    .execute(&pool)
    .await?;

    // Test
    let result = user::get_user_theme(&pool, user_id).await?;
    assert_eq!(result, "dark");

    Ok(())
}

#[sqlx::test]
async fn test_get_user_theme_returns_default_when_not_set(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let user_id = 999i64;
    let result = user::get_user_theme(&pool, user_id).await?;
    // Default should be "auto" (system preference)
    assert_eq!(result, "auto");
    Ok(())
}

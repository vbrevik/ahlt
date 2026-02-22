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

#[sqlx::test]
async fn test_set_user_theme_valid_light(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    // Create a user entity first
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO entities (entity_type, name, label) VALUES ('user', 'testuser_light', 'Test User Light') RETURNING id"
    )
    .fetch_one(&pool)
    .await?;

    // Set theme to "light"
    user::set_user_theme(&pool, user_id, "light").await?;

    // Verify it was saved and retrieves correctly
    let result = user::get_user_theme(&pool, user_id).await?;
    assert_eq!(result, "light");

    Ok(())
}

#[sqlx::test]
async fn test_set_user_theme_updates_existing(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    // Create a user entity first
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO entities (entity_type, name, label) VALUES ('user', 'testuser_update', 'Test User Update') RETURNING id"
    )
    .fetch_one(&pool)
    .await?;

    // Set initial theme
    user::set_user_theme(&pool, user_id, "light").await?;
    let result1 = user::get_user_theme(&pool, user_id).await?;
    assert_eq!(result1, "light");

    // Update to a different theme (tests ON CONFLICT behavior)
    user::set_user_theme(&pool, user_id, "dark").await?;
    let result2 = user::get_user_theme(&pool, user_id).await?;
    assert_eq!(result2, "dark");

    Ok(())
}

#[sqlx::test]
async fn test_set_user_theme_rejects_invalid_value(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    // Create a user entity first
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO entities (entity_type, name, label) VALUES ('user', 'testuser_invalid', 'Test User Invalid') RETURNING id"
    )
    .fetch_one(&pool)
    .await?;

    // Attempt to set an invalid theme value
    let result = user::set_user_theme(&pool, user_id, "invalid-theme").await;

    // Should return an error
    assert!(result.is_err());

    Ok(())
}

#[sqlx::test]
async fn test_set_user_theme_all_valid_values(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    // Create a user entity first
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO entities (entity_type, name, label) VALUES ('user', 'testuser_all_values', 'Test User All Values') RETURNING id"
    )
    .fetch_one(&pool)
    .await?;

    // Test all three valid theme values
    for theme in &["light", "dark", "auto"] {
        user::set_user_theme(&pool, user_id, theme).await?;
        let result = user::get_user_theme(&pool, user_id).await?;
        assert_eq!(&result, theme, "Failed to persist theme: {}", theme);
    }

    Ok(())
}

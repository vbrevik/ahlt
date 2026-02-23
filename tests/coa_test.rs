//! Integration tests for COA (Course of Action) model layer

mod common;

use ahlt::models::{coa, tor, user, agenda_point, relation};
use ahlt::models::user::NewUser;
use ahlt::auth::password;
use common::setup_test_db;

#[tokio::test]
async fn test_create_coa() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Setup: create a user as the COA author
    let user_id = user::create(pool, &NewUser {
        username: "coa_creator".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "coa_creator@test.com".to_string(),
        display_name: "COA Creator".to_string(),
    }).await.unwrap();

    // Create a simple COA
    let coa_id = coa::create(pool, "Increase Budget", "Allocate additional funding", "simple", user_id).await.unwrap();
    assert!(coa_id > 0);

    // Verify via find_by_id
    let detail = coa::find_by_id(pool, coa_id).await.unwrap();
    assert_eq!(detail.title, "Increase Budget");
    assert_eq!(detail.description, "Allocate additional funding");
    assert_eq!(detail.coa_type, "simple");
    assert_eq!(detail.created_by, user_id);
    assert!(detail.sections.is_empty());

    println!("[PASS] test_create_coa");
}

#[tokio::test]
async fn test_update_coa() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Setup
    let user_id = user::create(pool, &NewUser {
        username: "coa_updater".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "coa_updater@test.com".to_string(),
        display_name: "COA Updater".to_string(),
    }).await.unwrap();

    let coa_id = coa::create(pool, "Original Title", "Original description", "simple", user_id).await.unwrap();

    // Update title and description
    coa::update(pool, coa_id, "Revised Title", "Revised description").await.unwrap();

    // Verify changes
    let detail = coa::find_by_id(pool, coa_id).await.unwrap();
    assert_eq!(detail.title, "Revised Title");
    assert_eq!(detail.description, "Revised description");
    // coa_type should remain unchanged
    assert_eq!(detail.coa_type, "simple");

    println!("[PASS] test_update_coa");
}

#[tokio::test]
async fn test_find_coas_for_agenda_point() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Setup: user, ToR, agenda point
    let user_id = user::create(pool, &NewUser {
        username: "coa_ap_user".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "coa_ap_user@test.com".to_string(),
        display_name: "COA AP User".to_string(),
    }).await.unwrap();

    let tor_id = tor::create(pool, "CoaTestToR", "COA Test", &[
        ("description", "ToR for COA testing"),
        ("status", "active"),
        ("meeting_cadence", "weekly"),
        ("cadence_day", "Monday"),
        ("cadence_time", "09:00"),
        ("cadence_duration_minutes", "90"),
        ("default_location", "Room B"),
    ]).await.unwrap();

    let ap_id = agenda_point::create(
        pool,
        tor_id,
        "Budget Discussion",
        "Discuss budget options",
        "decision",
        "2026-03-01",
        30,
        user_id,
        "",
        "",
        "",
    ).await.unwrap();

    // Create two COAs and link to agenda point
    let coa1_id = coa::create(pool, "Option A", "Keep current budget", "simple", user_id).await.unwrap();
    let coa2_id = coa::create(pool, "Option B", "Increase budget 10%", "simple", user_id).await.unwrap();

    relation::create(pool, "considers_coa", ap_id, coa1_id).await.unwrap();
    relation::create(pool, "considers_coa", ap_id, coa2_id).await.unwrap();

    // Query COAs for this agenda point
    let coas = coa::find_all_for_agenda_point(pool, ap_id).await.unwrap();
    assert_eq!(coas.len(), 2);

    let titles: Vec<&str> = coas.iter().map(|c| c.title.as_str()).collect();
    assert!(titles.contains(&"Option A"));
    assert!(titles.contains(&"Option B"));

    println!("[PASS] test_find_coas_for_agenda_point");
}

#[tokio::test]
async fn test_add_section_to_coa() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Setup
    let user_id = user::create(pool, &NewUser {
        username: "coa_section_user".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "coa_section@test.com".to_string(),
        display_name: "COA Section User".to_string(),
    }).await.unwrap();

    // Create a complex COA
    let coa_id = coa::create(pool, "Restructure Plan", "Full restructuring proposal", "complex", user_id).await.unwrap();

    // Add two sections with explicit ordering
    let sec1_id = coa::add_section(pool, coa_id, "Phase 1", "Initial assessment", 1).await.unwrap();
    let sec2_id = coa::add_section(pool, coa_id, "Phase 2", "Implementation rollout", 2).await.unwrap();
    assert!(sec1_id > 0);
    assert!(sec2_id > 0);

    // Verify sections via find_sections
    let sections = coa::sections::find_sections(pool, coa_id).await.unwrap();
    assert_eq!(sections.len(), 2);

    // Sections should be ordered by order field
    assert_eq!(sections[0].title, "Phase 1");
    assert_eq!(sections[0].content, "Initial assessment");
    assert_eq!(sections[0].order, 1);

    assert_eq!(sections[1].title, "Phase 2");
    assert_eq!(sections[1].content, "Implementation rollout");
    assert_eq!(sections[1].order, 2);

    // Also verify via find_by_id (which embeds sections)
    let detail = coa::find_by_id(pool, coa_id).await.unwrap();
    assert_eq!(detail.coa_type, "complex");
    assert_eq!(detail.sections.len(), 2);
    assert_eq!(detail.sections[0].title, "Phase 1");
    assert_eq!(detail.sections[1].title, "Phase 2");

    println!("[PASS] test_add_section_to_coa");
}

#[tokio::test]
async fn test_find_by_id_not_found() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Use a bogus ID that cannot exist
    let result = coa::find_by_id(pool, 999999).await;
    assert!(result.is_err(), "find_by_id with non-existent ID should return an error");

    println!("[PASS] test_find_by_id_not_found");
}

use ahlt::models::{entity, relation, opinion};
mod common;
use common::{setup_test_db, insert_entity};

/// Insert opinion-related relation types not included in the base seed.
async fn seed_opinion_relation_types(pool: &sqlx::PgPool) {
    for rt in &["opinion_on", "opinion_by", "prefers_coa"] {
        sqlx::query(
            "INSERT INTO entities (entity_type, name, label) VALUES ('relation_type', $1, $1) \
             ON CONFLICT DO NOTHING",
        )
        .bind(rt)
        .execute(pool)
        .await
        .expect("Failed to seed opinion relation type");
    }
}

#[tokio::test]
async fn test_find_opinions_via_relations_only() {
    // Simulate seeded opinions: only relations, no entity_properties for recorded_by_id etc.
    let db = setup_test_db().await;
    let pool = db.pool();
    seed_opinion_relation_types(pool).await;

    let _tor_id = entity::create(pool, "tor", "test_tor", "Test ToR").await.unwrap();
    let ap_id = entity::create(pool, "agenda_point", "test_ap", "Test AP").await.unwrap();
    let coa_a_id = entity::create(pool, "coa", "coa_a", "COA A").await.unwrap();
    let coa_b_id = entity::create(pool, "coa", "coa_b", "COA B").await.unwrap();
    let user_alice = entity::create(pool, "user", "alice_test", "Alice Test").await.unwrap();
    let user_bob = entity::create(pool, "user", "bob_test", "Bob Test").await.unwrap();

    let op_alice = entity::create(pool, "opinion", "opinion_alice_test", "Alice opinion").await.unwrap();
    let op_bob = entity::create(pool, "opinion", "opinion_bob_test", "Bob opinion").await.unwrap();

    // opinion_on: opinion -> agenda_point
    relation::create(pool, "opinion_on", op_alice, ap_id).await.unwrap();
    relation::create(pool, "opinion_on", op_bob, ap_id).await.unwrap();

    // opinion_by: opinion -> user (seeded direction)
    relation::create(pool, "opinion_by", op_alice, user_alice).await.unwrap();
    relation::create(pool, "opinion_by", op_bob, user_bob).await.unwrap();

    // prefers_coa: opinion -> coa
    relation::create(pool, "prefers_coa", op_alice, coa_a_id).await.unwrap();
    relation::create(pool, "prefers_coa", op_bob, coa_b_id).await.unwrap();

    let opinions = opinion::find_opinions_for_agenda_point(pool, ap_id).await.unwrap();
    assert_eq!(opinions.len(), 2);

    let alice_op = opinions.iter().find(|o| o.recorded_by_name == "Alice Test");
    assert!(alice_op.is_some(), "Alice opinion not found by name");
    assert_eq!(alice_op.unwrap().preferred_coa_id, coa_a_id);

    let bob_op = opinions.iter().find(|o| o.recorded_by_name == "Bob Test");
    assert!(bob_op.is_some(), "Bob opinion not found by name");
    assert_eq!(bob_op.unwrap().preferred_coa_id, coa_b_id);
}

#[tokio::test]
async fn test_find_opinions_via_entity_properties_still_works() {
    // Ensure programmatic opinions (entity_properties) still work after query rewrite
    let db = setup_test_db().await;
    let pool = db.pool();
    seed_opinion_relation_types(pool).await;

    let ap_id = entity::create(pool, "agenda_point", "prog_ap", "Prog AP").await.unwrap();
    let coa_id = entity::create(pool, "coa", "prog_coa", "Prog COA").await.unwrap();
    let user_id = entity::create(pool, "user", "prog_user", "Prog User").await.unwrap();

    // Programmatic opinion creation
    opinion::record_opinion(pool, ap_id, user_id, coa_id, "My commentary").await.unwrap();

    let opinions = opinion::find_opinions_for_agenda_point(pool, ap_id).await.unwrap();
    assert_eq!(opinions.len(), 1);
    assert_eq!(opinions[0].recorded_by_name, "Prog User");
    assert_eq!(opinions[0].preferred_coa_id, coa_id);
    assert_eq!(opinions[0].commentary, "My commentary");
}

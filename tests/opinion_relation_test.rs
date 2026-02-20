use ahlt::models::{entity, relation, opinion};
mod common;

/// Insert opinion-related relation types not included in the base seed.
fn seed_opinion_relation_types(conn: &rusqlite::Connection) {
    for rt in &["opinion_on", "opinion_by", "prefers_coa"] {
        conn.execute(
            "INSERT OR IGNORE INTO entities (entity_type, name, label) VALUES ('relation_type', ?1, ?1)",
            [rt],
        )
        .expect("Failed to seed opinion relation type");
    }
}

#[test]
fn test_find_opinions_via_relations_only() {
    // Simulate seeded opinions: only relations, no entity_properties for recorded_by_id etc.
    let (_dir, conn) = common::setup_test_db();
    seed_opinion_relation_types(&conn);

    let _tor_id = entity::create(&conn, "tor", "test_tor", "Test ToR").unwrap();
    let ap_id = entity::create(&conn, "agenda_point", "test_ap", "Test AP").unwrap();
    let coa_a_id = entity::create(&conn, "coa", "coa_a", "COA A").unwrap();
    let coa_b_id = entity::create(&conn, "coa", "coa_b", "COA B").unwrap();
    let user_alice = entity::create(&conn, "user", "alice_test", "Alice Test").unwrap();
    let user_bob = entity::create(&conn, "user", "bob_test", "Bob Test").unwrap();

    let op_alice = entity::create(&conn, "opinion", "opinion_alice_test", "Alice opinion").unwrap();
    let op_bob = entity::create(&conn, "opinion", "opinion_bob_test", "Bob opinion").unwrap();

    // opinion_on: opinion -> agenda_point
    relation::create(&conn, "opinion_on", op_alice, ap_id).unwrap();
    relation::create(&conn, "opinion_on", op_bob, ap_id).unwrap();

    // opinion_by: opinion -> user (seeded direction)
    relation::create(&conn, "opinion_by", op_alice, user_alice).unwrap();
    relation::create(&conn, "opinion_by", op_bob, user_bob).unwrap();

    // prefers_coa: opinion -> coa
    relation::create(&conn, "prefers_coa", op_alice, coa_a_id).unwrap();
    relation::create(&conn, "prefers_coa", op_bob, coa_b_id).unwrap();

    let opinions = opinion::find_opinions_for_agenda_point(&conn, ap_id).unwrap();
    assert_eq!(opinions.len(), 2);

    let alice_op = opinions.iter().find(|o| o.recorded_by_name == "Alice Test");
    assert!(alice_op.is_some(), "Alice opinion not found by name");
    assert_eq!(alice_op.unwrap().preferred_coa_id, coa_a_id);

    let bob_op = opinions.iter().find(|o| o.recorded_by_name == "Bob Test");
    assert!(bob_op.is_some(), "Bob opinion not found by name");
    assert_eq!(bob_op.unwrap().preferred_coa_id, coa_b_id);
}

#[test]
fn test_find_opinions_via_entity_properties_still_works() {
    // Ensure programmatic opinions (entity_properties) still work after query rewrite
    let (_dir, conn) = common::setup_test_db();
    seed_opinion_relation_types(&conn);

    let ap_id = entity::create(&conn, "agenda_point", "prog_ap", "Prog AP").unwrap();
    let coa_id = entity::create(&conn, "coa", "prog_coa", "Prog COA").unwrap();
    let user_id = entity::create(&conn, "user", "prog_user", "Prog User").unwrap();

    // Programmatic opinion creation
    opinion::record_opinion(&conn, ap_id, user_id, coa_id, "My commentary").unwrap();

    let opinions = opinion::find_opinions_for_agenda_point(&conn, ap_id).unwrap();
    assert_eq!(opinions.len(), 1);
    assert_eq!(opinions[0].recorded_by_name, "Prog User");
    assert_eq!(opinions[0].preferred_coa_id, coa_id);
    assert_eq!(opinions[0].commentary, "My commentary");
}

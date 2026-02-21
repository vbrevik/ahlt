//! Workflow builder tests — covers workflow state machine configuration (statuses and transitions).
//!
//! Tests the workflow builder layer operations:
//! - Workflow scope listing and introspection
//! - Status creation, retrieval, updates, and deletion
//! - Transition creation, retrieval, updates, and deletion
//! - Validation (duplicate statuses, invalid status references, cascade constraints)

mod common;

use ahlt::models::workflow::*;
use common::*;

const TEST_SCOPE: &str = "test_workflow";
const TEST_STATUS_CODE: &str = "draft";
const TEST_STATUS_LABEL: &str = "Draft";
const TEST_TRANSITION_LABEL: &str = "Submit";

#[tokio::test]
async fn test_list_workflow_scopes_empty() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let scopes = list_workflow_scopes(pool)
        .await
        .expect("Failed to list scopes");

    // Fresh database should have no scopes
    assert!(scopes.is_empty());
}

#[tokio::test]
async fn test_create_status_success() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let status_id = create_status(pool, TEST_SCOPE, TEST_STATUS_CODE, TEST_STATUS_LABEL, 0, true, false)
        .await
        .expect("Failed to create status");

    assert!(status_id > 0);

    let statuses = list_statuses_for_scope(pool, TEST_SCOPE)
        .await
        .expect("Failed to list statuses");

    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].status_code, TEST_STATUS_CODE);
    assert_eq!(statuses[0].label, TEST_STATUS_LABEL);
    assert!(statuses[0].is_initial);
    assert!(!statuses[0].is_terminal);
}

#[tokio::test]
async fn test_create_status_duplicate() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let first_id = create_status(pool, TEST_SCOPE, TEST_STATUS_CODE, TEST_STATUS_LABEL, 0, true, false)
        .await
        .expect("Failed to create first status");
    assert!(first_id > 0);

    // Try to create status with same code in same scope
    let duplicate = create_status(pool, TEST_SCOPE, TEST_STATUS_CODE, "Different Label", 1, false, false).await;

    // Should fail on UNIQUE constraint
    assert!(duplicate.is_err());
}

#[tokio::test]
async fn test_list_statuses_for_scope() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Create multiple statuses
    let _ = create_status(pool, TEST_SCOPE, "draft", "Draft", 0, true, false)
        .await
        .expect("Failed to create draft status");
    let _ = create_status(pool, TEST_SCOPE, "active", "Active", 1, false, false)
        .await
        .expect("Failed to create active status");
    let _ = create_status(pool, TEST_SCOPE, "done", "Done", 2, false, true)
        .await
        .expect("Failed to create done status");

    let statuses = list_statuses_for_scope(pool, TEST_SCOPE)
        .await
        .expect("Failed to list statuses");

    assert_eq!(statuses.len(), 3);
    assert_eq!(statuses[0].status_code, "draft");
    assert_eq!(statuses[1].status_code, "active");
    assert_eq!(statuses[2].status_code, "done");
}

#[tokio::test]
async fn test_update_status_success() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let status_id = create_status(pool, TEST_SCOPE, TEST_STATUS_CODE, TEST_STATUS_LABEL, 0, true, false)
        .await
        .expect("Failed to create status");

    let new_label = "Draft (Pending Review)";
    let _ = update_status(pool, status_id, new_label, 1, false, false)
        .await
        .expect("Failed to update status");

    let statuses = list_statuses_for_scope(pool, TEST_SCOPE)
        .await
        .expect("Failed to list statuses");

    assert_eq!(statuses[0].label, new_label);
    assert!(!statuses[0].is_initial);
}

#[tokio::test]
async fn test_update_status_not_found() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let result = update_status(pool, 9999, "Updated Label", 0, true, false).await;

    // Updating non-existent status should error
    assert!(result.is_err());
}

#[tokio::test]
async fn test_create_transition_success() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let draft_id = create_status(pool, TEST_SCOPE, "draft", "Draft", 0, true, false)
        .await
        .expect("Failed to create draft status");
    let active_id = create_status(pool, TEST_SCOPE, "active", "Active", 1, false, false)
        .await
        .expect("Failed to create active status");

    let transition_id = create_transition(pool, TEST_SCOPE, draft_id, active_id, TEST_TRANSITION_LABEL, "", false, "")
        .await
        .expect("Failed to create transition");

    assert!(transition_id > 0);

    let transitions = list_transitions_for_scope(pool, TEST_SCOPE)
        .await
        .expect("Failed to list transitions");

    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].transition_label, TEST_TRANSITION_LABEL);
    assert_eq!(transitions[0].from_status_code, "draft");
    assert_eq!(transitions[0].to_status_code, "active");
}

#[tokio::test]
async fn test_create_transition_invalid_status() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let draft_id = create_status(pool, TEST_SCOPE, "draft", "Draft", 0, true, false)
        .await
        .expect("Failed to create draft status");

    // Try to create transition to non-existent status
    let result = create_transition(pool, TEST_SCOPE, draft_id, 9999, TEST_TRANSITION_LABEL, "", false, "").await;

    // Should fail when looking up target status properties
    assert!(result.is_err());
}

#[tokio::test]
async fn test_list_transitions_for_scope() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let draft_id = create_status(pool, TEST_SCOPE, "draft", "Draft", 0, true, false)
        .await
        .expect("Failed to create draft status");
    let active_id = create_status(pool, TEST_SCOPE, "active", "Active", 1, false, false)
        .await
        .expect("Failed to create active status");
    let done_id = create_status(pool, TEST_SCOPE, "done", "Done", 2, false, true)
        .await
        .expect("Failed to create done status");

    let _ = create_transition(pool, TEST_SCOPE, draft_id, active_id, "Submit", "", false, "")
        .await
        .expect("Failed to create draft->active transition");
    let _ = create_transition(pool, TEST_SCOPE, active_id, done_id, "Complete", "", false, "")
        .await
        .expect("Failed to create active->done transition");

    let transitions = list_transitions_for_scope(pool, TEST_SCOPE)
        .await
        .expect("Failed to list transitions");

    assert_eq!(transitions.len(), 2);
}

#[tokio::test]
async fn test_update_transition_success() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let draft_id = create_status(pool, TEST_SCOPE, "draft", "Draft", 0, true, false)
        .await
        .expect("Failed to create draft status");
    let active_id = create_status(pool, TEST_SCOPE, "active", "Active", 1, false, false)
        .await
        .expect("Failed to create active status");

    let transition_id = create_transition(pool, TEST_SCOPE, draft_id, active_id, "Submit", "", false, "")
        .await
        .expect("Failed to create transition");

    let new_label = "Submit for Review";
    let _ = update_transition(pool, transition_id, new_label, "permission.workflow.submit", false, "")
        .await
        .expect("Failed to update transition");

    let transitions = list_transitions_for_scope(pool, TEST_SCOPE)
        .await
        .expect("Failed to list transitions");

    assert_eq!(transitions[0].transition_label, new_label);
    assert_eq!(transitions[0].required_permission, "permission.workflow.submit");
}

#[tokio::test]
async fn test_delete_transition_success() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let draft_id = create_status(pool, TEST_SCOPE, "draft", "Draft", 0, true, false)
        .await
        .expect("Failed to create draft status");
    let active_id = create_status(pool, TEST_SCOPE, "active", "Active", 1, false, false)
        .await
        .expect("Failed to create active status");

    let transition_id = create_transition(pool, TEST_SCOPE, draft_id, active_id, "Submit", "", false, "")
        .await
        .expect("Failed to create transition");

    let _ = delete_transition(pool, transition_id)
        .await
        .expect("Failed to delete transition");

    let transitions = list_transitions_for_scope(pool, TEST_SCOPE)
        .await
        .expect("Failed to list transitions");

    assert!(transitions.is_empty());
}

#[tokio::test]
async fn test_delete_status_with_transitions() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let draft_id = create_status(pool, TEST_SCOPE, "draft", "Draft", 0, true, false)
        .await
        .expect("Failed to create draft status");
    let active_id = create_status(pool, TEST_SCOPE, "active", "Active", 1, false, false)
        .await
        .expect("Failed to create active status");

    let _ = create_transition(pool, TEST_SCOPE, draft_id, active_id, "Submit", "", false, "")
        .await
        .expect("Failed to create transition");

    // Try to delete status that has transitions pointing to it
    let result = delete_status(pool, active_id).await;

    // Should fail because status is referenced by a transition
    assert!(result.is_err());
}

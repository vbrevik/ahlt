use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::auth::csrf;
use crate::auth::session::{require_permission, get_user_id};
use crate::errors::{AppError, render};
use crate::models::{tor, agenda_point, coa, opinion};
use crate::models::opinion::{OpinionForm, DecisionForm};
use crate::templates_structs::{PageContext, OpinionFormTemplate, DecisionFormTemplate};

// ---------------------------------------------------------------------------
// Opinion Recording Handlers (Task 16)
// ---------------------------------------------------------------------------

/// GET /tor/{id}/workflow/agenda/{aid}/input
/// Renders the opinion recording form for an agenda point.
pub async fn form(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.participate")?;

    let (tor_id, agenda_point_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Fetch agenda point
    let agenda_point = agenda_point::find_by_id(&pool, agenda_point_id).await?
        .ok_or(AppError::NotFound)?;

    // Check that it's a decision-type agenda point
    if agenda_point.item_type != "decision" {
        return Err(AppError::PermissionDenied(
            "Opinions can only be recorded on decision-type agenda items".to_string(),
        ));
    }

    // Check if user has already recorded an opinion
    let existing_opinion = opinion::find_opinion_by_user_and_agenda_point(&pool, user_id, agenda_point_id).await?;

    // Load COAs for this agenda point
    let coas = coa::find_all_for_agenda_point(&pool, agenda_point_id).await?;

    // Load existing opinion if user already recorded one
    let opinion_detail = if let Some(opinion_id) = existing_opinion {
        opinion::find_opinion_by_id(&pool, opinion_id).await?
    } else {
        None
    };

    let ctx = PageContext::build(&session, &pool, "/workflow").await?;

    let tmpl = OpinionFormTemplate {
        ctx,
        tor_id,
        agenda_point_id,
        coas,
        existing_opinion: opinion_detail,
        errors: vec![],
    };
    render(tmpl)
}

/// POST /tor/{id}/workflow/agenda/{aid}/input
/// Records or updates an opinion on an agenda point.
pub async fn submit(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<OpinionForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.participate")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, agenda_point_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Validate form input
    let preferred_coa_id = form.preferred_coa_id;
    let commentary = form.commentary.trim();
    let mut errors = vec![];

    if preferred_coa_id <= 0 {
        errors.push("Please select a preferred course of action".to_string());
    }

    if !errors.is_empty() {
        let _agenda_point = agenda_point::find_by_id(&pool, agenda_point_id).await?
            .ok_or(AppError::NotFound)?;
        let coas = coa::find_all_for_agenda_point(&pool, agenda_point_id).await?;
        let ctx = PageContext::build(&session, &pool, "/workflow").await?;

        let existing_opinion = opinion::find_opinion_by_user_and_agenda_point(&pool, user_id, agenda_point_id).await?;
        let opinion_detail = if let Some(opinion_id) = existing_opinion {
            opinion::find_opinion_by_id(&pool, opinion_id).await?
        } else {
            None
        };

        let tmpl = OpinionFormTemplate {
            ctx,
            tor_id,
            agenda_point_id,
            coas,
            existing_opinion: opinion_detail,
            errors,
        };
        return render(tmpl);
    }

    // Check if user already has an opinion recorded
    let existing_opinion_id = opinion::find_opinion_by_user_and_agenda_point(&pool, user_id, agenda_point_id).await?;

    let opinion_id = if let Some(oid) = existing_opinion_id {
        // Update existing opinion
        opinion::update_opinion(&pool, oid, preferred_coa_id, commentary).await?;
        oid
    } else {
        // Create new opinion
        opinion::record_opinion(&pool, agenda_point_id, user_id, preferred_coa_id, commentary).await?
    };

    // Audit log
    let details = serde_json::json!({
        "agenda_point_id": agenda_point_id,
        "preferred_coa_id": preferred_coa_id,
        "commentary_length": commentary.len(),
        "summary": format!("Recorded opinion on agenda point #{} preferring COA #{}", agenda_point_id, preferred_coa_id)
    });
    let _ = crate::audit::log(&pool, user_id, "opinion.recorded", "opinion", opinion_id, details).await;

    let _ = session.insert("flash", "Opinion recorded successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}")))
        .finish())
}

// ---------------------------------------------------------------------------
// Decision Recording Handlers (Task 16)
// ---------------------------------------------------------------------------

/// GET /tor/{id}/workflow/agenda/{aid}/decide
/// Renders the decision recording form for an agenda point.
pub async fn decision_form(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.decide")?;

    let (tor_id, agenda_point_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Fetch agenda point
    let agenda_point = agenda_point::find_by_id(&pool, agenda_point_id).await?
        .ok_or(AppError::NotFound)?;

    // Check that agenda point status allows decision (not already "voted" or "completed")
    if agenda_point.status == "completed" {
        return Err(AppError::PermissionDenied(
            "A decision has already been recorded for this agenda item".to_string(),
        ));
    }

    // Load opinions summary
    let opinions_summary = opinion::get_opinions_summary(&pool, agenda_point_id).await?;

    // Convert summary to structured data
    let mut opinions_by_coa = std::collections::HashMap::new();
    for (coa_id, count) in opinions_summary {
        opinions_by_coa.insert(coa_id, count);
    }

    // Load all COAs for this agenda point
    let coa_list = coa::find_all_for_agenda_point(&pool, agenda_point_id).await?;
    let mut coas: Vec<coa::CoaDetail> = vec![];
    for c in coa_list.iter() {
        if let Ok(detail) = coa::find_by_id(&pool, c.id).await {
            coas.push(detail);
        }
    }

    // Build opinion summaries grouped by COA
    let mut opinions = vec![];
    for coa in &coas {
        let count = opinions_by_coa.get(&coa.id).copied().unwrap_or(0);
        let items = if count > 0 {
            opinion::find_opinions_for_agenda_point(&pool, agenda_point_id).await
                .unwrap_or_default()
                .into_iter()
                .filter(|o| o.preferred_coa_id == coa.id)
                .collect()
        } else {
            vec![]
        };

        opinions.push(opinion::OpinionSummary {
            coa_id: coa.id,
            coa_title: coa.title.clone(),
            preference_count: count,
            preference_pct: 0,
            opinions: items,
        });
    }

    let ctx = PageContext::build(&session, &pool, "/workflow").await?;

    let tmpl = DecisionFormTemplate {
        ctx,
        tor_id,
        agenda_point,
        coas,
        opinions,
        errors: vec![],
    };
    render(tmpl)
}

/// POST /tor/{id}/workflow/agenda/{aid}/decide
/// Records the final decision on an agenda point.
pub async fn record_decision(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<DecisionForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.decide")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, agenda_point_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Validate form input
    let selected_coa_id = form.selected_coa_id;
    let decision_rationale = form.decision_rationale.trim();
    let mut errors = vec![];

    if selected_coa_id <= 0 {
        errors.push("Please select a course of action".to_string());
    }

    if !errors.is_empty() {
        let agenda_point = agenda_point::find_by_id(&pool, agenda_point_id).await?
            .ok_or(AppError::NotFound)?;
        let coa_list = coa::find_all_for_agenda_point(&pool, agenda_point_id).await?;
        let mut coas: Vec<coa::CoaDetail> = vec![];
        for c in coa_list.iter() {
            if let Ok(detail) = coa::find_by_id(&pool, c.id).await {
                coas.push(detail);
            }
        }

        let opinions_summary = opinion::get_opinions_summary(&pool, agenda_point_id).await?;
        let mut opinions_by_coa = std::collections::HashMap::new();
        for (coa_id, count) in opinions_summary {
            opinions_by_coa.insert(coa_id, count);
        }

        let mut opinions = vec![];
        for coa in &coas {
            let count = opinions_by_coa.get(&coa.id).copied().unwrap_or(0);
            let items = if count > 0 {
                opinion::find_opinions_for_agenda_point(&pool, agenda_point_id).await
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|o| o.preferred_coa_id == coa.id)
                    .collect()
            } else {
                vec![]
            };

            opinions.push(opinion::OpinionSummary {
                coa_id: coa.id,
                coa_title: coa.title.clone(),
                preference_count: count,
                preference_pct: 0,
                opinions: items,
            });
        }

        let ctx = PageContext::build(&session, &pool, "/workflow").await?;

        let tmpl = DecisionFormTemplate {
            ctx,
            tor_id,
            agenda_point,
            coas,
            opinions,
            errors,
        };
        return render(tmpl);
    }

    // Record the decision
    let decision_id = opinion::record_decision(&pool, agenda_point_id, user_id, selected_coa_id, decision_rationale).await?;

    // Update agenda point with decision metadata
    let _ = crate::models::entity::set_property(&pool, agenda_point_id, "decided_by_id", &user_id.to_string()).await;
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let _ = crate::models::entity::set_property(&pool, agenda_point_id, "decided_date", &now).await;
    let _ = crate::models::entity::set_property(&pool, agenda_point_id, "selected_coa_id", &selected_coa_id.to_string()).await;

    // Audit log the decision
    let details = serde_json::json!({
        "agenda_point_id": agenda_point_id,
        "selected_coa_id": selected_coa_id,
        "rationale_length": decision_rationale.len(),
        "summary": format!("Recorded decision on agenda point #{} selecting COA #{}", agenda_point_id, selected_coa_id)
    });
    let _ = crate::audit::log(&pool, user_id, "decision.finalized", "decision", decision_id, details).await;

    let _ = session.insert("flash", "Decision recorded successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}")))
        .finish())
}

// src/models/table_filter/builder.rs

use std::collections::HashMap;
use super::{FilterTree, Group, Condition, Logic};

#[derive(Debug)]
pub enum BuildError {
    UnknownField(String),
    UnknownOp(String),
}

/// Build a parameterized WHERE fragment from a FilterTree.
/// Returns (sql_fragment, params_vec).
/// param_offset: the $N index to start from (so callers can combine with other params).
/// field_map: "field_key" -> "sql_col_expression" (must be pre-validated SQL, not user input)
/// op_whitelist: set of allowed operator strings for this table
pub fn build_where_clause(
    tree: &FilterTree,
    field_map: &HashMap<&str, &str>,
    op_whitelist: &[&str],
    param_offset: usize,
) -> Result<(String, Vec<String>), BuildError> {
    if tree.is_empty() {
        return Ok(("1=1".to_string(), vec![]));
    }
    let mut params: Vec<String> = vec![];
    let mut parts: Vec<String> = vec![];

    // Root-level conditions
    for cond in &tree.conditions {
        let (sql, mut p) = build_condition(cond, field_map, op_whitelist, param_offset + params.len())?;
        params.append(&mut p);
        parts.push(sql);
    }

    // Groups (one level deep)
    for group in &tree.groups {
        let (sql, mut p) = build_group(group, field_map, op_whitelist, param_offset + params.len())?;
        if !sql.is_empty() {
            params.append(&mut p);
            parts.push(format!("({sql})"));
        }
    }

    if parts.is_empty() {
        return Ok(("1=1".to_string(), vec![]));
    }

    let logic = match tree.logic { Logic::And => " AND ", Logic::Or => " OR " };
    Ok((parts.join(logic), params))
}

fn build_group(
    group: &Group,
    field_map: &HashMap<&str, &str>,
    op_whitelist: &[&str],
    param_offset: usize,
) -> Result<(String, Vec<String>), BuildError> {
    let mut params: Vec<String> = vec![];
    let mut parts: Vec<String> = vec![];
    for cond in &group.conditions {
        let (sql, mut p) = build_condition(cond, field_map, op_whitelist, param_offset + params.len())?;
        params.append(&mut p);
        parts.push(sql);
    }
    if parts.is_empty() { return Ok(("".to_string(), vec![])); }
    let logic = match group.logic { Logic::And => " AND ", Logic::Or => " OR " };
    Ok((parts.join(logic), params))
}

fn build_condition(
    cond: &Condition,
    field_map: &HashMap<&str, &str>,
    op_whitelist: &[&str],
    param_offset: usize,
) -> Result<(String, Vec<String>), BuildError> {
    let col = field_map.get(cond.field.as_str())
        .ok_or_else(|| BuildError::UnknownField(cond.field.clone()))?;
    if !op_whitelist.contains(&cond.op.as_str()) {
        return Err(BuildError::UnknownOp(cond.op.clone()));
    }
    let n = param_offset + 1;  // PostgreSQL uses 1-based $N
    let (sql, value) = match cond.op.as_str() {
        "contains"     => (format!("{col} LIKE '%' || ${n} || '%'"), cond.value.clone()),
        "not_contains" => (format!("{col} NOT LIKE '%' || ${n} || '%'"), cond.value.clone()),
        "equals" | "is"         => (format!("{col} = ${n}"), cond.value.clone()),
        "not_equals" | "is_not" => (format!("{col} != ${n}"), cond.value.clone()),
        "starts_with"  => (format!("{col} LIKE ${n} || '%'"), cond.value.clone()),
        "before"       => (format!("{col} < ${n}"), cond.value.clone()),
        "after"        => (format!("{col} > ${n}"), cond.value.clone()),
        "on"           => (format!("{col}::DATE = (${n})::DATE"), cond.value.clone()),
        _ => return Err(BuildError::UnknownOp(cond.op.clone())),
    };
    Ok((sql, vec![value]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::table_filter::{Condition, FilterTree, Group, Logic};

    fn user_field_map() -> HashMap<&'static str, &'static str> {
        let mut m = HashMap::new();
        m.insert("username", "e.name");
        m.insert("email", "COALESCE(p_email.value, '')");
        m.insert("role", "COALESCE(role_e.name, '')");
        m
    }

    const USER_OPS: &[&str] = &[
        "contains", "not_contains", "equals", "not_equals",
        "starts_with", "is", "is_not", "before", "after", "on",
    ];

    #[test]
    fn empty_tree_returns_passthrough() {
        let (sql, params) = build_where_clause(
            &FilterTree::default(), &user_field_map(), USER_OPS, 0
        ).unwrap();
        assert_eq!(sql, "1=1");
        assert!(params.is_empty());
    }

    #[test]
    fn single_contains_condition() {
        let tree = FilterTree {
            conditions: vec![Condition { field: "username".into(), op: "contains".into(), value: "alice".into() }],
            ..Default::default()
        };
        let (sql, params) = build_where_clause(&tree, &user_field_map(), USER_OPS, 0).unwrap();
        assert_eq!(sql, "e.name LIKE '%' || $1 || '%'");
        assert_eq!(params, vec!["alice"]);
    }

    #[test]
    fn root_and_two_conditions() {
        let tree = FilterTree {
            logic: Logic::And,
            conditions: vec![
                Condition { field: "username".into(), op: "contains".into(), value: "alice".into() },
                Condition { field: "role".into(), op: "is".into(), value: "admin".into() },
            ],
            ..Default::default()
        };
        let (sql, params) = build_where_clause(&tree, &user_field_map(), USER_OPS, 0).unwrap();
        assert_eq!(sql, "e.name LIKE '%' || $1 || '%' AND COALESCE(role_e.name, '') = $2");
        assert_eq!(params, vec!["alice", "admin"]);
    }

    #[test]
    fn condition_with_nested_or_group() {
        let tree = FilterTree {
            logic: Logic::And,
            conditions: vec![
                Condition { field: "role".into(), op: "is".into(), value: "admin".into() },
            ],
            groups: vec![Group {
                logic: Logic::Or,
                conditions: vec![
                    Condition { field: "username".into(), op: "contains".into(), value: "alice".into() },
                    Condition { field: "email".into(), op: "contains".into(), value: "@acme".into() },
                ],
            }],
        };
        let (sql, params) = build_where_clause(&tree, &user_field_map(), USER_OPS, 0).unwrap();
        assert_eq!(
            sql,
            "COALESCE(role_e.name, '') = $1 AND (e.name LIKE '%' || $2 || '%' OR COALESCE(p_email.value, '') LIKE '%' || $3 || '%')"
        );
        assert_eq!(params, vec!["admin", "alice", "@acme"]);
    }

    #[test]
    fn unknown_field_returns_error() {
        let tree = FilterTree {
            conditions: vec![Condition { field: "nonexistent".into(), op: "equals".into(), value: "x".into() }],
            ..Default::default()
        };
        assert!(build_where_clause(&tree, &user_field_map(), USER_OPS, 0).is_err());
    }

    #[test]
    fn param_offset_shifts_placeholders() {
        let tree = FilterTree {
            conditions: vec![Condition { field: "username".into(), op: "contains".into(), value: "alice".into() }],
            ..Default::default()
        };
        let (sql, params) = build_where_clause(&tree, &user_field_map(), USER_OPS, 5).unwrap();
        assert_eq!(sql, "e.name LIKE '%' || $6 || '%'");
        assert_eq!(params, vec!["alice"]);
    }
}

// src/models/user/filter.rs
use std::collections::HashMap;

/// SQL column expressions for each filter field.
/// Values are hardcoded SQL expressions — never user input.
pub fn field_map() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("username",     "e.name");
    m.insert("display_name", "e.label");
    m.insert("email",        "COALESCE(p_email.value, '')");
    m.insert("role",         "COALESCE(role_e.name, '')");
    m.insert("created_at",   "e.created_at");
    m.insert("updated_at",   "e.updated_at");
    m
}

/// Allowed operators for the users table.
pub const OPS: &[&str] = &[
    "contains", "not_contains", "equals", "not_equals",
    "starts_with", "is", "is_not", "before", "after", "on",
];

/// Allowed sort column keys and their SQL expressions.
pub fn sort_col(key: &str) -> &'static str {
    match key {
        "username"     => "e.name",
        "display_name" => "e.label",
        "email"        => "COALESCE(p_email.value, '')",
        "role"         => "COALESCE(role_e.name, '')",
        "created_at"   => "e.created_at",
        "updated_at"   => "e.updated_at",
        _              => "e.id",
    }
}

/// Default column definitions for the users table (order = default display order).
pub fn default_columns() -> Vec<crate::models::table_filter::ColumnDef> {
    use crate::models::table_filter::ColumnDef;
    vec![
        ColumnDef { key: "user".into(),       label: "User".into(),    visible: true,  always_visible: true,  sortable: true,  sort_key: "username".into() },
        ColumnDef { key: "email".into(),      label: "Email".into(),   visible: true,  always_visible: false, sortable: true,  sort_key: "email".into() },
        ColumnDef { key: "status".into(),     label: "Status".into(),  visible: true,  always_visible: false, sortable: false, sort_key: "".into() },
        ColumnDef { key: "created_at".into(), label: "Created".into(), visible: false, always_visible: false, sortable: true,  sort_key: "created_at".into() },
        ColumnDef { key: "updated_at".into(), label: "Updated".into(), visible: false, always_visible: false, sortable: true,  sort_key: "updated_at".into() },
        ColumnDef { key: "actions".into(),    label: "Actions".into(), visible: true,  always_visible: true,  sortable: false, sort_key: "".into() },
    ]
}

/// Field definitions for the filter builder JS (as JSON).
/// Includes label, type, and allowed operators.
/// role_options: Vec<(name, label)> fetched from DB.
pub fn fields_json(role_options: &[(String, String)]) -> String {
    let roles_json: String = role_options.iter()
        .map(|(name, label)| format!(r#"{{"value":"{name}","label":"{label}"}}"#))
        .collect::<Vec<_>>()
        .join(",");

    format!(r#"[
  {{"key":"username","label":"Username","type":"text","ops":["contains","not_contains","equals","not_equals","starts_with"]}},
  {{"key":"display_name","label":"Display Name","type":"text","ops":["contains","not_contains","equals","not_equals","starts_with"]}},
  {{"key":"email","label":"Email","type":"text","ops":["contains","not_contains","equals","not_equals"]}},
  {{"key":"role","label":"Role","type":"select","ops":["is","is_not"],"options":[{roles_json}]}},
  {{"key":"created_at","label":"Created","type":"date","ops":["before","after","on"]}},
  {{"key":"updated_at","label":"Updated","type":"date","ops":["before","after","on"]}}
]"#)
}

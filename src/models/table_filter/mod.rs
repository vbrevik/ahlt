use serde::{Deserialize, Serialize};

pub mod builder;
pub mod columns;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Logic { #[default] And, Or }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,
    pub op: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Group {
    #[serde(default)]
    pub logic: Logic,
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FilterTree {
    #[serde(default)]
    pub logic: Logic,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default)]
    pub groups: Vec<Group>,
}

impl FilterTree {
    pub fn is_empty(&self) -> bool {
        self.conditions.is_empty() && self.groups.is_empty()
    }
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum SortDir { #[default] Asc, Desc }

#[derive(Debug, Clone, Default)]
pub struct SortSpec {
    pub column: String,
    pub dir: SortDir,
}

impl SortSpec {
    pub fn from_params(sort: Option<&str>, dir: Option<&str>) -> Self {
        SortSpec {
            column: sort.unwrap_or("").to_string(),
            dir: if dir == Some("desc") { SortDir::Desc } else { SortDir::Asc },
        }
    }
    pub fn dir_str(&self) -> &'static str {
        match self.dir { SortDir::Asc => "asc", SortDir::Desc => "desc" }
    }
    pub fn toggle_dir(&self) -> &'static str {
        match self.dir { SortDir::Asc => "desc", SortDir::Desc => "asc" }
    }
}

/// Ordered column definition passed to templates.
#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub key: String,
    pub label: String,
    pub visible: bool,
    pub always_visible: bool,
    pub sortable: bool,
    pub sort_key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_tree_empty_roundtrip() {
        let tree = FilterTree::default();
        let json = tree.to_json();
        let back = FilterTree::from_json(&json).unwrap();
        assert!(back.is_empty());
    }

    #[test]
    fn filter_tree_full_roundtrip() {
        let tree = FilterTree {
            logic: Logic::And,
            conditions: vec![Condition {
                field: "username".into(),
                op: "contains".into(),
                value: "alice".into(),
            }],
            groups: vec![Group {
                logic: Logic::Or,
                conditions: vec![
                    Condition { field: "email".into(), op: "contains".into(), value: "@acme".into() },
                    Condition { field: "role".into(), op: "is".into(), value: "admin".into() },
                ],
            }],
        };
        let json = tree.to_json();
        let back = FilterTree::from_json(&json).unwrap();
        assert_eq!(back.conditions.len(), 1);
        assert_eq!(back.groups.len(), 1);
        assert_eq!(back.groups[0].conditions.len(), 2);
        assert_eq!(back.logic, Logic::And);
        assert_eq!(back.groups[0].logic, Logic::Or);
    }

    #[test]
    fn sort_spec_from_params() {
        let s = SortSpec::from_params(Some("username"), Some("desc"));
        assert_eq!(s.column, "username");
        assert_eq!(s.dir, SortDir::Desc);
        assert_eq!(s.toggle_dir(), "asc");
    }

    #[test]
    fn sort_spec_defaults_when_none() {
        let s = SortSpec::from_params(None, None);
        assert_eq!(s.column, "");
        assert_eq!(s.dir, SortDir::Asc);
        assert_eq!(s.dir_str(), "asc");
        assert_eq!(s.toggle_dir(), "desc");
    }

    #[test]
    fn sort_spec_asc_dir() {
        let s = SortSpec::from_params(Some("email"), Some("asc"));
        assert_eq!(s.column, "email");
        assert_eq!(s.dir, SortDir::Asc);
    }
}

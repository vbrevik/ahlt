#[derive(Debug, Clone)]
pub struct ProtocolStep {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub step_type: String,       // "procedural", "agenda_slot", "fixed"
    pub sequence_order: i64,
    pub default_duration_minutes: Option<i64>,
    pub description: String,
    pub is_required: bool,
    pub responsible: String,
}

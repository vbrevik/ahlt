#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PresentationTemplate {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub description: String,
    pub slide_count: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TemplateSlide {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub slide_order: i64,
    pub required_content: String,  // description of what this slide should contain
    pub notes: String,
}

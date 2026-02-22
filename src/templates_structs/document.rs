use askama::Template;

use super::PageContext;

#[derive(Template)]
#[template(path = "documents/list.html")]
pub struct DocumentListTemplate {
    pub ctx: PageContext,
    pub documents: Vec<crate::models::document::DocumentListItem>,
    pub search_query: String,  // Empty string if no search
    pub total_count: i64,
}

#[derive(Template)]
#[template(path = "documents/form.html")]
pub struct DocumentFormTemplate {
    pub ctx: PageContext,
    pub form_title: String,
    pub form_action: String,
    pub document: Option<crate::models::document::DocumentDetail>,
    pub errors: Vec<String>,
}

#[derive(Template)]
#[template(path = "documents/detail.html")]
pub struct DocumentDetailTemplate {
    pub ctx: PageContext,
    pub document: crate::models::document::DocumentDetail,
}

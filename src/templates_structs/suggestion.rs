use askama::Template;

use super::PageContext;

#[derive(Template)]
#[template(path = "suggestions/form.html")]
pub struct SuggestionFormTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub tor_name: String,
    pub errors: Vec<String>,
}

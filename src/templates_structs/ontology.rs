use askama::Template;

use crate::models::ontology::{EntityTypeSummary, RelationTypeSummary, EntityDetail};
use super::PageContext;

#[derive(Template)]
#[template(path = "ontology/concepts.html")]
pub struct OntologyConceptsTemplate {
    pub ctx: PageContext,
    pub entity_types: Vec<EntityTypeSummary>,
    pub relation_types: Vec<RelationTypeSummary>,
}

#[derive(Template)]
#[template(path = "ontology/graph.html")]
pub struct OntologyGraphTemplate {
    pub ctx: PageContext,
}

#[derive(Template)]
#[template(path = "ontology/data.html")]
pub struct OntologyDataTemplate {
    pub ctx: PageContext,
}

#[derive(Template)]
#[template(path = "ontology/detail.html")]
pub struct OntologyDetailTemplate {
    pub ctx: PageContext,
    pub entity: EntityDetail,
}

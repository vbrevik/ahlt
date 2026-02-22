// Template context structures for Askama templates, organized by domain.
// All types are re-exported for backward compatibility: `use ahlt::templates_structs::*`

mod common;
mod user;
mod role;
mod dashboard;
mod audit;
mod ontology;
mod tor;
mod workflow;
mod suggestion;
mod proposal;
mod agenda;
mod coa;
mod opinion;
mod meeting;
mod warning;
mod document;
mod api;

// Re-export all types for seamless imports
pub use self::common::{PageContext, TorContext, LoginTemplate, AccountTemplate, SettingsTemplate, DataManagerTemplate};
pub use self::user::{UserListTemplate, UserFormTemplate, ApiUserResponse, ApiUserRequest};
pub use self::role::{
    RoleAssignmentTemplate, MatrixCell, PermissionRow, PageGroup, RoleColumn, MenuBuilderTemplate,
    PermissionGroup, RoleBuilderTemplate, PreviewRequest, PreviewResponse, RoleBuilderForm,
};
pub use self::dashboard::DashboardTemplate;
pub use self::audit::AuditListTemplate;
pub use self::ontology::{OntologyConceptsTemplate, OntologyGraphTemplate, OntologyDataTemplate, OntologyDetailTemplate};
pub use self::tor::{
    TorListTemplate, TorFormTemplate, UserOption, TorDetailTemplate, GovernanceMapTemplate,
    TorOutlookTemplate, PresentationTemplatesTemplate,
};
pub use self::workflow::{
    WorkflowTemplate, WorkflowIndexTemplate, WorkflowBuilderListTemplate,
    WorkflowBuilderDetailTemplate, QueueTemplate,
};
pub use self::suggestion::SuggestionFormTemplate;
pub use self::proposal::{ProposalFormTemplate, ProposalDetailTemplate};
pub use self::agenda::{AgendaPointFormTemplate, AgendaPointDetailTemplate};
pub use self::coa::CoaFormTemplate;
pub use self::opinion::{OpinionFormTemplate, DecisionFormTemplate};
pub use self::meeting::{
    MeetingsListTemplate, TorMeetingsListTemplate, MeetingDetailTemplate, MinutesViewTemplate,
};
pub use self::warning::{WarningListTemplate, WarningDetailTemplate};
pub use self::document::{DocumentListTemplate, DocumentFormTemplate, DocumentDetailTemplate};
pub use self::api::{
    PaginatedResponse, ApiEntityProperty, ApiEntityResponse, ApiEntityRequest, ApiErrorResponse,
};

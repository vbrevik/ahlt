/// Meeting CRUD handlers (Create, Read, Update operations).
///
/// This module organizes meeting handlers by operation type:
/// - `read.rs`: GET detail view
/// - `create.rs`: POST confirm, confirm_calendar
/// - `update.rs`: POST transition, agenda management, minutes generation, roll call
/// - `forms.rs`: Form structures for deserialization (shared across handlers)
/// - `helpers.rs`: Shared validation and ToR boundary checks
///
/// All handlers are re-exported at module level for backwards compatibility.
/// Route registration remains unchanged in main.rs.

pub mod forms;
pub mod helpers;
pub mod read;
pub mod create;
pub mod update;

// Re-exports for backwards compatibility
pub use read::detail;
pub use create::{confirm, confirm_calendar};
pub use update::{
    transition, assign_agenda, remove_agenda, generate_minutes, save_roll_call,
};
pub use forms::{
    ConfirmForm, CalendarConfirmForm, TransitionForm, AgendaForm, CsrfOnly, RollCallForm,
};

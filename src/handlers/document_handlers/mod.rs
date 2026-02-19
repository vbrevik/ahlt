pub mod list;
pub mod crud;

pub use list::list;
pub use crud::{new_form, create, detail, edit_form, update, delete};

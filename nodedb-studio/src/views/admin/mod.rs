//! Administration: access, row policies, the audit trail, backups, server
//! config, and credentials.

pub mod access;
pub mod access_details;
pub mod access_list;
pub mod access_panels;
pub mod access_sheets;
pub mod audit;
pub mod audit_details;
pub mod backup_details;
pub mod backup_sheets;
pub mod backups;
pub mod common;
pub mod config;
pub mod config_review;
pub mod cred_edit;
pub mod credentials;
pub mod page;
pub mod policies;
pub mod policy_editor;
pub mod policy_test;
pub mod tasks;

pub use page::AdminPage;

//! [`CommandName`]: a scoped command identifier.
//!
//! Spec 010, decision 010-5: commands are scoped to the plugin that defines
//! them rather than living in one flat namespace, so two different plugins
//! defining a command called `open` is not a collision at all — it is
//! structurally impossible for one to be, because `plugin.open` and
//! `other.open` are different names. `plugin: None` names a shell built-in,
//! which has no owning plugin to qualify it.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A command name, optionally qualified by the plugin that defines it.
///
/// Renders as `plugin.name` when qualified, or bare `name` for a shell
/// built-in (`plugin: None`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandName {
    /// The plugin that defines this command, or `None` for a shell
    /// built-in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    /// The command's own name, unqualified.
    pub name: String,
}

impl CommandName {
    /// A shell built-in command, carrying no plugin qualifier.
    #[must_use]
    pub fn builtin(name: impl Into<String>) -> Self {
        Self {
            plugin: None,
            name: name.into(),
        }
    }

    /// A command defined by `plugin`.
    #[must_use]
    pub fn scoped(plugin: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            plugin: Some(plugin.into()),
            name: name.into(),
        }
    }
}

impl fmt::Display for CommandName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.plugin {
            Some(plugin) => write!(f, "{plugin}.{}", self.name),
            None => f.write_str(&self.name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_displays_bare() {
        assert_eq!(CommandName::builtin("save").to_string(), "save");
    }

    #[test]
    fn scoped_displays_qualified() {
        assert_eq!(CommandName::scoped("git", "open").to_string(), "git.open");
    }

    #[test]
    fn two_plugins_may_share_a_command_name() {
        let a = CommandName::scoped("git", "open");
        let b = CommandName::scoped("files", "open");
        assert_ne!(a, b);
        assert_eq!(a.name, b.name);
        assert_eq!(a.to_string(), "git.open");
        assert_eq!(b.to_string(), "files.open");
    }

    #[test]
    fn serde_round_trips_both_shapes() {
        let scoped = CommandName::scoped("git", "open");
        let json = serde_json::to_string(&scoped).unwrap();
        assert_eq!(json, r#"{"plugin":"git","name":"open"}"#);
        assert_eq!(serde_json::from_str::<CommandName>(&json).unwrap(), scoped);

        let builtin = CommandName::builtin("save");
        let json = serde_json::to_string(&builtin).unwrap();
        assert_eq!(json, r#"{"name":"save"}"#);
        assert_eq!(serde_json::from_str::<CommandName>(&json).unwrap(), builtin);
    }
}

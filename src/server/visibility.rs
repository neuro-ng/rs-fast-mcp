//! Hierarchical component visibility filter.
//!
//! `VisibilityFilter` replaces the per-component `enabled: bool` flag with a
//! centralised, server-level allow/block system that supports key-based and
//! tag-based rules.

use std::collections::HashSet;

/// Declarative visibility rule applied to a set of components.
///
/// Rules are evaluated lazily when a listing request arrives; components
/// are never mutated.
#[derive(Debug, Clone)]
pub struct Visibility {
    /// Whether this rule enables (`true`) or disables (`false`) matching components.
    pub enabled: bool,
    /// Specific component names (tool name, resource URI, prompt name) to match.
    pub names: Option<HashSet<String>>,
    /// Stable component keys to match (see `Component::key`).
    pub keys: Option<HashSet<String>>,
    /// Tags to match (at least one tag must match when set).
    pub tags: Option<HashSet<String>>,
    /// When `true` every condition in this rule must match simultaneously.
    pub match_all: bool,
}

impl Visibility {
    /// Convenience constructor to enable a set of named components.
    pub fn enable_names(names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            enabled: true,
            names: Some(names.into_iter().map(Into::into).collect()),
            keys: None,
            tags: None,
            match_all: false,
        }
    }

    /// Convenience constructor to disable a set of named components.
    pub fn disable_names(names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            enabled: false,
            names: Some(names.into_iter().map(Into::into).collect()),
            keys: None,
            tags: None,
            match_all: false,
        }
    }
}

/// Server-level filter that decides whether a component should appear in
/// listing responses.
///
/// By default all components are visible. Calling [`disable`] narrows
/// visibility; [`enable`] can then re-open specific items inside a
/// previously-disabled set.
///
/// Rules are evaluated in insertion order; later rules override earlier ones
/// when they address the same component.
#[derive(Debug, Clone)]
pub struct VisibilityFilter {
    disabled_keys: HashSet<String>,
    disabled_tags: HashSet<String>,
    enabled_keys: HashSet<String>,
    enabled_tags: HashSet<String>,
    /// `true` = all components are visible unless explicitly disabled.
    pub default_enabled: bool,
}

impl Default for VisibilityFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl VisibilityFilter {
    pub fn new() -> Self {
        Self {
            disabled_keys: HashSet::new(),
            disabled_tags: HashSet::new(),
            enabled_keys: HashSet::new(),
            enabled_tags: HashSet::new(),
            default_enabled: true,
        }
    }

    /// Add keys and/or tags to the allow-list.
    ///
    /// If `only` is `true` the default visibility is flipped to *disabled* so
    /// that only explicitly enabled items are shown.
    pub fn enable(
        &mut self,
        keys: Option<HashSet<String>>,
        tags: Option<HashSet<String>>,
        only: bool,
    ) {
        if only {
            self.default_enabled = false;
        }
        if let Some(ks) = keys {
            self.enabled_keys.extend(ks);
        }
        if let Some(ts) = tags {
            self.enabled_tags.extend(ts);
        }
    }

    /// Add keys and/or tags to the block-list.
    pub fn disable(&mut self, keys: Option<HashSet<String>>, tags: Option<HashSet<String>>) {
        if let Some(ks) = keys {
            self.disabled_keys.extend(ks);
        }
        if let Some(ts) = tags {
            self.disabled_tags.extend(ts);
        }
    }

    /// Returns whether a component with the given `key` and `tags` is visible.
    ///
    /// Evaluation order:
    /// 1. Explicit block-list wins if the key or any tag is disabled.
    /// 2. Explicit allow-list wins if the key or any tag is enabled.
    /// 3. Fall back to `default_enabled`.
    pub fn is_enabled(&self, key: &str, tags: &HashSet<String>) -> bool {
        // Block-list check
        if self.disabled_keys.contains(key) {
            return false;
        }
        for tag in tags {
            if self.disabled_tags.contains(tag) {
                return false;
            }
        }

        // Allow-list check
        if self.enabled_keys.contains(key) {
            return true;
        }
        for tag in tags {
            if self.enabled_tags.contains(tag) {
                return true;
            }
        }

        self.default_enabled
    }
}

/// Session-scoped visibility rules applied on top of the server-level filter.
#[derive(Debug, Clone, Default)]
pub struct SessionVisibility {
    rules: Vec<Visibility>,
}

impl SessionVisibility {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_rule(&mut self, rule: Visibility) {
        self.rules.push(rule);
    }

    /// Apply the accumulated rules: returns `false` if any disable-rule matches
    /// and no subsequent enable-rule overrides it.
    pub fn is_enabled(&self, name: &str, tags: &HashSet<String>) -> bool {
        let mut result = true;
        for rule in &self.rules {
            let name_match = rule
                .names
                .as_ref()
                .map(|ns| ns.contains(name))
                .unwrap_or(false);
            let tag_match = rule
                .tags
                .as_ref()
                .map(|ts| tags.iter().any(|t| ts.contains(t)))
                .unwrap_or(false);

            if name_match || tag_match {
                result = rule.enabled;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_all_enabled() {
        let f = VisibilityFilter::new();
        assert!(f.is_enabled("tool1", &HashSet::new()));
    }

    #[test]
    fn test_disable_key() {
        let mut f = VisibilityFilter::new();
        f.disable(Some(["tool1".to_string()].into()), None);
        assert!(!f.is_enabled("tool1", &HashSet::new()));
        assert!(f.is_enabled("tool2", &HashSet::new()));
    }

    #[test]
    fn test_enable_only_mode() {
        let mut f = VisibilityFilter::new();
        f.enable(Some(["tool1".to_string()].into()), None, true);
        assert!(f.is_enabled("tool1", &HashSet::new()));
        assert!(!f.is_enabled("tool2", &HashSet::new()));
    }

    #[test]
    fn test_disable_tag() {
        let mut f = VisibilityFilter::new();
        f.disable(None, Some(["internal".to_string()].into()));
        let mut tags = HashSet::new();
        tags.insert("internal".to_string());
        assert!(!f.is_enabled("anything", &tags));
        assert!(f.is_enabled("anything", &HashSet::new()));
    }

    #[test]
    fn test_session_visibility_override() {
        let mut sv = SessionVisibility::new();
        sv.add_rule(Visibility::disable_names(["tool1"]));
        sv.add_rule(Visibility::enable_names(["tool1"]));
        assert!(sv.is_enabled("tool1", &HashSet::new()));
    }
}

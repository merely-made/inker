//! Ephemeral Micron field state. Parsing, editing and preparation perform no IO.
//!
//! Hosts bind this state to a document identity and address, resolve native
//! targets, and require an explicit submission action. The source check here
//! cannot establish destination authority or cancel a host's network request.
use super::syntax::{self, FieldKind, LineKind, LinkEffect, Span};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormLimits {
    /// UTF-8 source bytes accepted before syntax parsing.
    pub max_source_bytes: usize,
    /// Maximum field declarations, submit actions, and submitted map entries.
    pub max_fields: usize,
    /// UTF-8 bytes in an individual edited, initial, or fixed value.
    pub max_value_bytes: usize,
    /// Sum of submitted UTF-8 key/value bytes. The transport must separately
    /// enforce its complete encoded envelope and link packet limits.
    pub max_submission_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = include_str!("../../tests/fixtures/micron/forms/index.mu");

    fn state() -> FormState {
        FormState::from_source(SOURCE, FormLimits::default()).unwrap()
    }
    fn expected(json: &str) -> BTreeMap<String, String> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn stock_client_default_and_edited_maps() {
        let mut form = state();
        assert_eq!(
            form.prepare(0, SOURCE).unwrap().values(),
            &expected(include_str!(
                "../../tests/fixtures/micron/forms/defaults.json"
            ))
        );
        form.set_text(0, "edited café 雪".into()).unwrap();
        form.set_text(7, "first line\nsecond 雪".into()).unwrap();
        assert_eq!(
            form.prepare(0, SOURCE).unwrap().values(),
            &expected(include_str!(
                "../../tests/fixtures/micron/forms/edited.json"
            ))
        );
        assert!(form.matches_source(SOURCE));
        assert!(form.fields()[2].masked());
    }

    #[test]
    fn stock_client_selected_fields_and_radio_exclusion() {
        let mut form = state();
        form.set_checked(3, false).unwrap();
        form.set_checked(6, true).unwrap();
        assert!(!form.fields()[5].checked());
        assert_eq!(
            form.prepare(1, SOURCE).unwrap().values(),
            &expected(include_str!(
                "../../tests/fixtures/micron/forms/selected.json"
            ))
        );
    }

    #[test]
    fn navigation_and_invalid_edits_cannot_reuse_prepared_state() {
        let mut form = state();
        assert!(form.prepare(0, "replacement page").is_err());
        assert!(form.prepare(99, SOURCE).is_err());
        assert!(form.set_checked(0, true).is_err());
        assert!(form.set_text(3, "bad".into()).is_err());
        assert!(form.set_text(99, "bad".into()).is_err());
        assert_eq!(
            form.prepare(0, SOURCE).unwrap().values(),
            &expected(include_str!(
                "../../tests/fixtures/micron/forms/defaults.json"
            ))
        );
    }

    #[test]
    fn configurable_limits_apply_before_submission() {
        assert!(
            FormState::from_source(
                SOURCE,
                FormLimits {
                    max_source_bytes: 5,
                    ..FormLimits::default()
                }
            )
            .is_err()
        );
        assert!(
            FormState::from_source(
                SOURCE,
                FormLimits {
                    max_fields: 1,
                    ..FormLimits::default()
                }
            )
            .is_err()
        );
        assert!(
            FormState::from_source(
                SOURCE,
                FormLimits {
                    max_value_bytes: 1,
                    ..FormLimits::default()
                }
            )
            .is_err()
        );
        let mut form = FormState::from_source(
            SOURCE,
            FormLimits {
                max_submission_bytes: 1,
                ..FormLimits::default()
            },
        )
        .unwrap();
        assert!(form.prepare(0, SOURCE).is_err());
        assert!(form.set_text(0, "x".repeat(4097)).is_err());
        assert_eq!(form.fields()[0].value(), "seed-text");
    }

    #[test]
    fn ambiguous_or_incomplete_forms_are_refused() {
        for source in [
            "`<name`a>\n`<name`b>",
            "`<?|name|a`>\n`<^|name|b`>",
            "`<^|name|a|*`>\n`<^|name|b|*`>",
            "`<?|name|a`>\n`<?|name|a`>",
            "`[Send`:/page/test`*]\n`t\n| `<name`a> |\n`t",
        ] {
            assert!(
                FormState::from_source(source, FormLimits::default()).is_err(),
                "{source}"
            );
        }
        for source in ["`[Send`:/page/test`missing]", "`[Send`:/page/test`x=a|x=b]"] {
            assert!(
                FormState::from_source(source, FormLimits::default())
                    .unwrap()
                    .prepare(0, source)
                    .is_err()
            );
        }
    }

    #[test]
    fn debug_does_not_disclose_field_or_fixed_values() {
        let source = "`<!|secret`do-not-log>\n`[Send`:/page/test`*|token=also-private]";
        let form = FormState::from_source(source, FormLimits::default()).unwrap();
        let debug = format!(
            "{form:?} {:?} {:?}",
            form.actions(),
            form.prepare(0, source).unwrap()
        );
        assert!(!debug.contains("do-not-log"));
        assert!(!debug.contains("also-private"));
    }
}
impl Default for FormLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 1024 * 1024,
            max_fields: 64,
            max_value_bytes: 4096,
            max_submission_bytes: 16384,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct FormAction {
    pub label: String,
    pub target: String,
    pub selectors: String,
}
impl std::fmt::Debug for FormAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FormAction")
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Field {
    pub kind: FieldKind,
    value: String,
    checked: bool,
}
impl Field {
    pub fn name(&self) -> &str {
        match &self.kind {
            FieldKind::Text { name, .. }
            | FieldKind::Checkbox { name, .. }
            | FieldKind::Radio { name, .. } => name,
        }
    }
    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn checked(&self) -> bool {
        self.checked
    }
    pub fn masked(&self) -> bool {
        matches!(self.kind, FieldKind::Text { masked: true, .. })
    }
}
impl std::fmt::Debug for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Field")
            .field("name", &self.name())
            .field("value", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PreparedForm {
    pub target: String,
    values: BTreeMap<String, String>,
}
impl PreparedForm {
    pub fn values(&self) -> &BTreeMap<String, String> {
        &self.values
    }
    pub fn into_values(self) -> BTreeMap<String, String> {
        self.values
    }
}
impl std::fmt::Debug for PreparedForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedForm")
            .field("target", &self.target)
            .field("field_count", &self.values.len())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct FormState {
    source: String,
    fields: Vec<Field>,
    actions: Vec<FormAction>,
    limits: FormLimits,
}
impl std::fmt::Debug for FormState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FormState")
            .field("fields", &self.fields)
            .field("action_count", &self.actions.len())
            .finish()
    }
}
impl FormState {
    pub fn from_source(source: &str, limits: FormLimits) -> Result<Self, String> {
        if source.len() > limits.max_source_bytes {
            return Err("Micron form source exceeds the configured limit".into());
        }
        let mut state = Self {
            source: source.into(),
            fields: Vec::new(),
            actions: Vec::new(),
            limits,
        };
        let mut unsupported_controls = false;
        for line in syntax::parse(source).lines {
            // Table bodies are not exposed as parsed fields yet. Never silently
            // submit only the fields outside them, or outside a dynamic partial.
            if let LineKind::Table { body, .. } = &line.kind {
                unsupported_controls |= body.contains("`<") || body.contains("`[");
            }
            for span in line.spans {
                match span {
                    Span::Field { kind, initial, .. } => {
                        if state.fields.len() >= limits.max_fields
                            || initial.len() > limits.max_value_bytes
                        {
                            return Err("Micron form fields exceed the configured limit".into());
                        }
                        let (value, checked) = match &kind {
                            FieldKind::Text { .. } => (initial, false),
                            FieldKind::Checkbox {
                                value, prechecked, ..
                            }
                            | FieldKind::Radio {
                                value, prechecked, ..
                            } => (value.clone(), *prechecked),
                        };
                        if value.len() > limits.max_value_bytes {
                            return Err("Micron choice exceeds the configured limit".into());
                        }
                        let field = Field {
                            kind,
                            value,
                            checked,
                        };
                        if field.name().is_empty() {
                            return Err("Micron field name is empty".into());
                        }
                        for previous in state.fields.iter().filter(|p| p.name() == field.name()) {
                            let compatible = matches!(
                                (&previous.kind, &field.kind),
                                (FieldKind::Checkbox { .. }, FieldKind::Checkbox { .. })
                                    | (FieldKind::Radio { .. }, FieldKind::Radio { .. })
                            );
                            if !compatible
                                || previous.value == field.value
                                || (matches!(field.kind, FieldKind::Radio { .. })
                                    && previous.checked
                                    && checked)
                            {
                                return Err("Ambiguous Micron field group".into());
                            }
                        }
                        state.fields.push(field);
                    },
                    Span::Link {
                        label,
                        target,
                        effect: Some(LinkEffect::RequestSelectors(selectors)),
                        ..
                    } => {
                        if state.actions.len() >= limits.max_fields {
                            return Err("Too many Micron submit actions".into());
                        }
                        state.actions.push(FormAction {
                            label,
                            target,
                            selectors,
                        });
                    },
                    Span::Partial { .. } | Span::Unsupported(_) => unsupported_controls = true,
                    _ => {},
                }
            }
        }
        if unsupported_controls && !state.actions.is_empty() {
            return Err("This form contains controls that are not supported yet".into());
        }
        Ok(state)
    }
    pub fn matches_source(&self, source: &str) -> bool {
        self.source == source
    }
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }
    pub fn actions(&self) -> &[FormAction] {
        &self.actions
    }
    pub fn set_text(&mut self, index: usize, value: String) -> Result<(), String> {
        if value.len() > self.limits.max_value_bytes {
            return Err("Field exceeds the configured byte limit".into());
        }
        let field = self.fields.get_mut(index).ok_or("Unknown Micron field")?;
        if !matches!(field.kind, FieldKind::Text { .. }) {
            return Err("This field is a choice".into());
        }
        field.value = value;
        Ok(())
    }
    pub fn set_checked(&mut self, index: usize, checked: bool) -> Result<(), String> {
        let field = self.fields.get(index).ok_or("Unknown Micron field")?;
        match &field.kind {
            FieldKind::Text { .. } => return Err("This field is text".into()),
            FieldKind::Radio { name, .. } if checked => {
                let name = name.clone();
                for field in &mut self.fields {
                    if field.name() == name {
                        field.checked = false;
                    }
                }
            },
            _ => {},
        }
        self.fields[index].checked = checked;
        Ok(())
    }
    pub fn prepare(&self, index: usize, current_source: &str) -> Result<PreparedForm, String> {
        if !self.matches_source(current_source) {
            return Err("The page changed; reopen its form".into());
        }
        let action = self
            .actions
            .get(index)
            .ok_or("Unknown Micron submit action")?;
        let mut selected = BTreeSet::new();
        let mut values = BTreeMap::new();
        for selector in action.selectors.split('|') {
            if selector == "*" {
                selected.extend(self.fields.iter().map(|f| f.name().to_owned()));
            } else if let Some((name, value)) = selector.split_once('=') {
                if value.len() > self.limits.max_value_bytes {
                    return Err("Fixed variable exceeds the configured byte limit".into());
                }
                if name.is_empty()
                    || values
                        .insert(format!("var_{name}"), value.to_owned())
                        .is_some()
                {
                    return Err("Ambiguous fixed Micron variable".into());
                }
            } else {
                if !self.fields.iter().any(|f| f.name() == selector) {
                    return Err("Unknown Micron field selector".into());
                }
                selected.insert(selector.to_owned());
            }
        }
        for name in selected {
            let group: Vec<_> = self.fields.iter().filter(|f| f.name() == name).collect();
            if !matches!(group[0].kind, FieldKind::Text { .. })
                && !group.iter().any(|field| field.checked)
            {
                return Err(
                    "An unselected choice group is not qualified for submission yet".into(),
                );
            }
            let value = match &group[0].kind {
                FieldKind::Text { .. } => group[0].value.clone(),
                _ => group
                    .iter()
                    .filter(|f| f.checked)
                    .map(|f| f.value.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
            };
            values.insert(format!("field_{name}"), value);
        }
        let bytes = values.iter().try_fold(0usize, |n, (k, v)| {
            n.checked_add(k.len())?.checked_add(v.len())
        });
        if values.len() > self.limits.max_fields
            || bytes.is_none_or(|n| n > self.limits.max_submission_bytes)
        {
            return Err("Submission exceeds the configured field/byte limit".into());
        }
        Ok(PreparedForm {
            target: action.target.clone(),
            values,
        })
    }
}

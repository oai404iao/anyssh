use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use zeroize::Zeroizing;

use crate::{
    StorageError,
    entity_id::{generate_opaque_id, is_valid_opaque_id},
};

pub const MAX_SNIPPETS: usize = 256;
pub const MAX_SNIPPET_LABEL_BYTES: usize = 128;
pub const MAX_SNIPPET_BODY_BYTES: usize = 64 * 1024;
pub const MAX_SNIPPET_VARIABLES: usize = 16;
pub const MAX_SNIPPET_VARIABLE_NAME_BYTES: usize = 32;
pub const MAX_SNIPPET_VARIABLE_VALUE_BYTES: usize = 4 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnippetSummary {
    id: String,
    label: String,
    variables: Vec<String>,
    line_count: u32,
    updated_at: i64,
}

impl SnippetSummary {
    pub(crate) fn new(
        id: String,
        label: String,
        variables: Vec<String>,
        line_count: u32,
        updated_at: i64,
    ) -> Result<Self, StorageError> {
        if !valid_snippet_id(&id)
            || !valid_label(&label)
            || variables.len() > MAX_SNIPPET_VARIABLES
            || variables.iter().any(|name| !valid_variable_name(name))
            || variables.windows(2).any(|names| names[0] >= names[1])
            || line_count == 0
            || updated_at < 0
        {
            return Err(StorageError::InvalidSnippet);
        }
        Ok(Self {
            id,
            label,
            variables,
            line_count,
            updated_at,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn variables(&self) -> &[String] {
        &self.variables
    }

    pub const fn line_count(&self) -> u32 {
        self.line_count
    }

    pub const fn updated_at(&self) -> i64 {
        self.updated_at
    }
}

pub struct SnippetDraft {
    summary: SnippetSummary,
    body: Zeroizing<String>,
}

impl SnippetDraft {
    pub(crate) fn new(
        id: String,
        label: String,
        body: Zeroizing<String>,
        updated_at: i64,
    ) -> Result<Self, StorageError> {
        let (variables, line_count) = parse_snippet_body(body.as_str())?;
        let summary = SnippetSummary::new(id, label, variables, line_count, updated_at)?;
        Ok(Self { summary, body })
    }

    pub fn summary(&self) -> &SnippetSummary {
        &self.summary
    }

    pub fn body(&self) -> &str {
        self.body.as_str()
    }

    pub fn into_body(self) -> Zeroizing<String> {
        self.body
    }

    pub fn render(
        &self,
        values: &BTreeMap<String, String>,
    ) -> Result<Zeroizing<String>, StorageError> {
        render_snippet(self.body(), self.summary.variables(), values)
    }
}

impl fmt::Debug for SnippetDraft {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SnippetDraft")
            .field("summary", &self.summary)
            .field("body", &"<redacted>")
            .finish()
    }
}

pub(crate) struct SnippetRecord {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) body: Zeroizing<String>,
    pub(crate) variables: Vec<String>,
    pub(crate) line_count: u32,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
}

impl SnippetRecord {
    pub(crate) fn generate(
        label: String,
        body: Zeroizing<String>,
        now: i64,
    ) -> Result<Self, StorageError> {
        Self::new(generate_opaque_id("snippet-")?, label, body, now, now)
    }

    pub(crate) fn new(
        id: String,
        label: String,
        body: Zeroizing<String>,
        created_at: i64,
        updated_at: i64,
    ) -> Result<Self, StorageError> {
        let (variables, line_count) = parse_snippet_body(body.as_str())?;
        if !valid_snippet_id(&id)
            || !valid_label(&label)
            || created_at < 0
            || updated_at < created_at
        {
            return Err(StorageError::InvalidSnippet);
        }
        Ok(Self {
            id,
            label,
            body,
            variables,
            line_count,
            created_at,
            updated_at,
        })
    }

    pub(crate) fn summary(&self) -> SnippetSummary {
        SnippetSummary {
            id: self.id.clone(),
            label: self.label.clone(),
            variables: self.variables.clone(),
            line_count: self.line_count,
            updated_at: self.updated_at,
        }
    }
}

pub(crate) fn valid_snippet_id(id: &str) -> bool {
    id.starts_with("snippet-") && is_valid_opaque_id(id)
}

pub(crate) fn parse_snippet_body(body: &str) -> Result<(Vec<String>, u32), StorageError> {
    if body.is_empty()
        || body.len() > MAX_SNIPPET_BODY_BYTES
        || body.chars().any(|character| {
            character == '\0'
                || (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        })
    {
        return Err(StorageError::InvalidSnippet);
    }

    let mut variables = BTreeSet::new();
    let bytes = body.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"{{") {
            let name_start = index + 2;
            let Some(relative_end) = body[name_start..].find("}}") else {
                return Err(StorageError::InvalidSnippet);
            };
            let name_end = name_start + relative_end;
            let name = &body[name_start..name_end];
            if !valid_variable_name(name)
                || body[name_start..name_end].contains("{{")
                || variables.len() == MAX_SNIPPET_VARIABLES && !variables.contains(name)
            {
                return Err(StorageError::InvalidSnippet);
            }
            variables.insert(name.to_owned());
            index = name_end + 2;
            continue;
        }
        if bytes[index..].starts_with(b"}}") {
            return Err(StorageError::InvalidSnippet);
        }
        index += 1;
    }

    let line_count =
        u32::try_from(body.split('\n').count()).map_err(|_| StorageError::InvalidSnippet)?;
    Ok((variables.into_iter().collect(), line_count))
}

fn render_snippet(
    body: &str,
    variables: &[String],
    values: &BTreeMap<String, String>,
) -> Result<Zeroizing<String>, StorageError> {
    if values.len() != variables.len()
        || variables.iter().any(|name| !values.contains_key(name))
        || values.keys().any(|name| !variables.contains(name))
        || values
            .values()
            .any(|value| !valid_variable_value(value.as_str()))
    {
        return Err(StorageError::InvalidSnippetVariables);
    }

    let mut rendered = Zeroizing::new(String::with_capacity(body.len()));
    let mut remaining = body;
    while let Some(start) = remaining.find("{{") {
        rendered.push_str(&remaining[..start]);
        let name_start = start + 2;
        let end = remaining[name_start..]
            .find("}}")
            .ok_or(StorageError::RecordIntegrity)?
            + name_start;
        let name = &remaining[name_start..end];
        rendered.push_str(
            values
                .get(name)
                .ok_or(StorageError::InvalidSnippetVariables)?,
        );
        if rendered.len() > MAX_SNIPPET_BODY_BYTES {
            return Err(StorageError::InvalidSnippetVariables);
        }
        remaining = &remaining[end + 2..];
    }
    rendered.push_str(remaining);
    if rendered.is_empty() || rendered.len() > MAX_SNIPPET_BODY_BYTES {
        return Err(StorageError::InvalidSnippetVariables);
    }
    Ok(rendered)
}

fn valid_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= MAX_SNIPPET_LABEL_BYTES
        && label.trim() == label
        && !label.chars().any(char::is_control)
}

fn valid_variable_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_SNIPPET_VARIABLE_NAME_BYTES
        && name.bytes().enumerate().all(|(index, byte)| {
            if index == 0 {
                byte.is_ascii_alphabetic()
            } else {
                byte.is_ascii_alphanumeric() || byte == b'_'
            }
        })
}

fn valid_variable_value(value: &str) -> bool {
    value.len() <= MAX_SNIPPET_VARIABLE_VALUE_BYTES
        && !value.chars().any(|character| {
            character == '\0'
                || (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_returns_sorted_unique_variables() {
        let (variables, lines) = parse_snippet_body("echo {{host}}\nprintf '%s' {{host}} {{port}}")
            .expect("parse snippet");
        assert_eq!(variables, ["host", "port"]);
        assert_eq!(lines, 2);
    }

    #[test]
    fn literal_render_requires_the_exact_variable_set() {
        let draft = SnippetDraft::new(
            "snippet-test".to_owned(),
            "Connect".to_owned(),
            Zeroizing::new("ssh {{user}}@{{host}}".to_owned()),
            1,
        )
        .expect("snippet");
        let values = BTreeMap::from([
            ("host".to_owned(), "server.example".to_owned()),
            ("user".to_owned(), "alice".to_owned()),
        ]);
        assert_eq!(
            draft.render(&values).expect("render").as_str(),
            "ssh alice@server.example"
        );

        let missing = BTreeMap::from([("host".to_owned(), "server.example".to_owned())]);
        assert!(matches!(
            draft.render(&missing),
            Err(StorageError::InvalidSnippetVariables)
        ));
    }

    #[test]
    fn debug_redacts_the_body() {
        let draft = SnippetDraft::new(
            "snippet-debug".to_owned(),
            "Debug".to_owned(),
            Zeroizing::new("do-not-log-this-command".to_owned()),
            1,
        )
        .expect("snippet");
        let debug = format!("{draft:?}");
        assert!(!debug.contains("do-not-log-this-command"));
        assert!(debug.contains("<redacted>"));
    }
}

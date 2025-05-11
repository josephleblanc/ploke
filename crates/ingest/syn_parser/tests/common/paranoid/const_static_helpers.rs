use ploke_core::{TrackingHash, TypeId};
use syn_parser::parser::{
    nodes::{Attribute, ConstNode, ConstNodeId, VisibilityKind},
    utils::ExtractSpan, // Assuming ExtractSpan is needed or relevant
};

/// Builder for creating `ConstNode` instances, primarily for testing.
#[derive(Default, Debug, Clone)]
pub struct ConstNodeBuilder {
    id: Option<ConstNodeId>,
    name: Option<String>,
    span: Option<(usize, usize)>,
    visibility: Option<VisibilityKind>,
    type_id: Option<TypeId>,
    value: Option<String>,               // Optional value expression string
    attributes: Vec<Attribute>,          // Defaults to empty vec
    docstring: Option<String>,           // Optional docstring
    tracking_hash: Option<TrackingHash>, // Optional hash
}

impl ConstNodeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(mut self, id: ConstNodeId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn span(mut self, span: (usize, usize)) -> Self {
        self.span = Some(span);
        self
    }

    /// Set span from an item implementing ExtractSpan
    pub fn span_from(mut self, item: &impl ExtractSpan) -> Self {
        self.span = Some(item.extract_span_bytes());
        self
    }

    pub fn visibility(mut self, visibility: VisibilityKind) -> Self {
        self.visibility = Some(visibility);
        self
    }

    pub fn type_id(mut self, type_id: TypeId) -> Self {
        self.type_id = Some(type_id);
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn add_attribute(mut self, attribute: Attribute) -> Self {
        self.attributes.push(attribute);
        self
    }

    pub fn attributes(mut self, attributes: Vec<Attribute>) -> Self {
        self.attributes = attributes;
        self
    }

    pub fn docstring(mut self, docstring: impl Into<String>) -> Self {
        self.docstring = Some(docstring.into());
        self
    }

    pub fn tracking_hash(mut self, hash: TrackingHash) -> Self {
        self.tracking_hash = Some(hash);
        self
    }

    /// Builds the `ConstNode`.
    ///
    /// # Errors
    /// Returns an error string if any required fields (`id`, `name`, `span`, `visibility`, `type_id`) are missing.
    pub fn build(self) -> Result<ConstNode, String> {
        let id = self.id.ok_or("Missing required field: id")?;
        let name = self.name.ok_or("Missing required field: name")?;
        let span = self.span.ok_or("Missing required field: span")?;
        let visibility = self
            .visibility
            .ok_or("Missing required field: visibility")?;
        let type_id = self.type_id.ok_or("Missing required field: type_id")?;

        Ok(ConstNode {
            id,
            name,
            span,
            visibility,
            type_id,
            value: self.value,
            attributes: self.attributes,
            docstring: self.docstring,
            tracking_hash: self.tracking_hash,
        })
    }
}

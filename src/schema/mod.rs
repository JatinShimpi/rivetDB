pub mod validator;

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Field types for schema validation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
    Array(Box<FieldType>),
    Object(Schema),
    Optional(Box<FieldType>),
    Any,
}

impl FieldType {
    /// Parse field type from string
    pub fn from_str(s: &str) -> Result<Self, String> {
        let s = s.trim().to_lowercase();
        
        // Handle optional types (ending with ?)
        if s.ends_with('?') {
            let inner = &s[..s.len() - 1];
            return Ok(FieldType::Optional(Box::new(FieldType::from_str(inner)?)));
        }
        
        // Handle array types
        if s.starts_with("array<") && s.ends_with('>') {
            let inner = &s[6..s.len() - 1];
            return Ok(FieldType::Array(Box::new(FieldType::from_str(inner)?)));
        }
        
        match s.as_str() {
            "string" | "str" => Ok(FieldType::String),
            "integer" | "int" | "i64" => Ok(FieldType::Integer),
            "float" | "f64" | "number" => Ok(FieldType::Float),
            "boolean" | "bool" => Ok(FieldType::Boolean),
            "any" => Ok(FieldType::Any),
            _ => Err(format!("Unknown field type: {}", s)),
        }
    }
    
    /// Convert to display string
    pub fn to_string(&self) -> String {
        match self {
            FieldType::String => "string".to_string(),
            FieldType::Integer => "integer".to_string(),
            FieldType::Float => "float".to_string(),
            FieldType::Boolean => "boolean".to_string(),
            FieldType::Array(inner) => format!("array<{}>", inner.to_string()),
            FieldType::Object(schema) => format!("object<{}>", schema.name),
            FieldType::Optional(inner) => format!("{}?", inner.to_string()),
            FieldType::Any => "any".to_string(),
        }
    }
}

/// Schema definition for typed objects
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub fields: HashMap<String, FieldType>,
    pub strict: bool, // If true, extra fields are not allowed
}

impl Schema {
    /// Create a new schema
    pub fn new(name: String) -> Self {
        Schema {
            name,
            fields: HashMap::new(),
            strict: true,
        }
    }
    
    /// Add a field to the schema
    pub fn add_field(&mut self, name: String, field_type: FieldType) {
        self.fields.insert(name, field_type);
    }
    
    /// Check if schema has a field
    pub fn has_field(&self, name: &str) -> bool {
        self.fields.contains_key(name)
    }
    
    /// Get field type
    pub fn get_field(&self, name: &str) -> Option<&FieldType> {
        self.fields.get(name)
    }
    
    /// Get all field names
    pub fn field_names(&self) -> Vec<&String> {
        self.fields.keys().collect()
    }
    
    /// Parse schema from JSON-like definition
    /// Format: {"field1": "type1", "field2": "type2"}
    pub fn from_definition(name: &str, definition: &str) -> Result<Self, String> {
        let mut schema = Schema::new(name.to_string());
        
        // Parse JSON-like format
        let definition = definition.trim();
        if !definition.starts_with('{') || !definition.ends_with('}') {
            return Err("Schema definition must be enclosed in braces {}".to_string());
        }
        
        let inner = &definition[1..definition.len() - 1];
        if inner.trim().is_empty() {
            return Ok(schema);
        }
        
        // Split by comma, but handle nested structures
        for pair in split_fields(inner) {
            let pair = pair.trim();
            if pair.is_empty() {
                continue;
            }
            
            // Find the colon separating key and value
            let colon_pos = pair.find(':')
                .ok_or_else(|| format!("Invalid field definition: {}", pair))?;
            
            let field_name = pair[..colon_pos].trim();
            let field_type = pair[colon_pos + 1..].trim();
            
            // Remove quotes from field name if present
            let field_name = field_name.trim_matches('"').trim_matches('\'');
            
            // Remove quotes from type if present
            let field_type = field_type.trim_matches('"').trim_matches('\'');
            
            schema.add_field(
                field_name.to_string(),
                FieldType::from_str(field_type)?,
            );
        }
        
        Ok(schema)
    }
    
    /// Convert schema to JSON-like string
    pub fn to_definition(&self) -> String {
        let fields: Vec<String> = self.fields
            .iter()
            .map(|(name, field_type)| format!("\"{}\": \"{}\"", name, field_type.to_string()))
            .collect();
        
        format!("{{{}}}", fields.join(", "))
    }
}

/// Split fields by comma, handling nested braces and quotes
fn split_fields(s: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut brace_depth = 0;
    let mut in_quotes = false;
    let mut quote_char = ' ';
    
    for c in s.chars() {
        match c {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote_char = c;
                current.push(c);
            }
            c if c == quote_char && in_quotes => {
                in_quotes = false;
                current.push(c);
            }
            '{' | '[' if !in_quotes => {
                brace_depth += 1;
                current.push(c);
            }
            '}' | ']' if !in_quotes => {
                brace_depth -= 1;
                current.push(c);
            }
            ',' if !in_quotes && brace_depth == 0 => {
                fields.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(c);
            }
        }
    }
    
    if !current.trim().is_empty() {
        fields.push(current.trim().to_string());
    }
    
    fields
}

/// Typed value - stores both the raw JSON and validated type info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedValue {
    pub schema_name: String,
    pub data: serde_json::Value,
}

impl TypedValue {
    pub fn new(schema_name: String, data: serde_json::Value) -> Self {
        TypedValue { schema_name, data }
    }
    
    /// Get a field value
    pub fn get(&self, field: &str) -> Option<&serde_json::Value> {
        self.data.get(field)
    }
    
    /// Convert to JSON string
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.data).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_field_type_from_str() {
        assert_eq!(FieldType::from_str("string").unwrap(), FieldType::String);
        assert_eq!(FieldType::from_str("integer").unwrap(), FieldType::Integer);
        assert_eq!(FieldType::from_str("float").unwrap(), FieldType::Float);
        assert_eq!(FieldType::from_str("boolean").unwrap(), FieldType::Boolean);
        
        // Optional
        assert_eq!(
            FieldType::from_str("string?").unwrap(),
            FieldType::Optional(Box::new(FieldType::String))
        );
        
        // Array
        assert_eq!(
            FieldType::from_str("array<integer>").unwrap(),
            FieldType::Array(Box::new(FieldType::Integer))
        );
    }
    
    #[test]
    fn test_schema_from_definition() {
        let schema = Schema::from_definition("user", r#"{"id": "integer", "name": "string"}"#).unwrap();
        
        assert_eq!(schema.name, "user");
        assert_eq!(schema.fields.len(), 2);
        assert_eq!(schema.get_field("id"), Some(&FieldType::Integer));
        assert_eq!(schema.get_field("name"), Some(&FieldType::String));
    }
}

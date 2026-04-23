use super::{FieldType, Schema};
use serde_json::Value;

/// Validation error with details
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub expected: String,
    pub actual: String,
    pub message: String,
}

impl ValidationError {
    pub fn new(field: &str, expected: &str, actual: &str) -> Self {
        ValidationError {
            field: field.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
            message: format!(
                "Field '{}': expected {}, got {}",
                field, expected, actual
            ),
        }
    }
    
    pub fn missing_field(field: &str, expected_type: &str) -> Self {
        ValidationError {
            field: field.to_string(),
            expected: expected_type.to_string(),
            actual: "missing".to_string(),
            message: format!("Missing required field '{}' of type {}", field, expected_type),
        }
    }
    
    pub fn extra_field(field: &str) -> Self {
        ValidationError {
            field: field.to_string(),
            expected: "none".to_string(),
            actual: "present".to_string(),
            message: format!("Unexpected field '{}' (strict mode)", field),
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Validate a JSON value against a schema
pub fn validate(value: &Value, schema: &Schema) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    
    // Value must be an object
    let obj = match value.as_object() {
        Some(o) => o,
        None => {
            errors.push(ValidationError::new("_root", "object", value_type_name(value)));
            return Err(errors);
        }
    };
    
    // Check all required fields
    for (field_name, field_type) in &schema.fields {
        match obj.get(field_name) {
            Some(field_value) => {
                if let Err(e) = validate_field(field_name, field_value, field_type) {
                    errors.push(e);
                }
            }
            None => {
                // Check if field is optional
                if !matches!(field_type, FieldType::Optional(_)) {
                    errors.push(ValidationError::missing_field(
                        field_name,
                        &field_type.to_string(),
                    ));
                }
            }
        }
    }
    
    // Check for extra fields in strict mode
    if schema.strict {
        for key in obj.keys() {
            if !schema.has_field(key) {
                errors.push(ValidationError::extra_field(key));
            }
        }
    }
    
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate a single field value against its expected type
fn validate_field(
    field_name: &str,
    value: &Value,
    expected_type: &FieldType,
) -> Result<(), ValidationError> {
    match expected_type {
        FieldType::String => {
            if !value.is_string() {
                return Err(ValidationError::new(
                    field_name,
                    "string",
                    value_type_name(value),
                ));
            }
        }
        FieldType::Integer => {
            if !value.is_i64() && !value.is_u64() {
                // Also accept whole numbers as floats
                if let Some(f) = value.as_f64() {
                    if f.fract() != 0.0 {
                        return Err(ValidationError::new(
                            field_name,
                            "integer",
                            "float",
                        ));
                    }
                } else {
                    return Err(ValidationError::new(
                        field_name,
                        "integer",
                        value_type_name(value),
                    ));
                }
            }
        }
        FieldType::Float => {
            if !value.is_number() {
                return Err(ValidationError::new(
                    field_name,
                    "float",
                    value_type_name(value),
                ));
            }
        }
        FieldType::Boolean => {
            if !value.is_boolean() {
                return Err(ValidationError::new(
                    field_name,
                    "boolean",
                    value_type_name(value),
                ));
            }
        }
        FieldType::Array(inner_type) => {
            if let Some(arr) = value.as_array() {
                for (i, item) in arr.iter().enumerate() {
                    let item_field = format!("{}[{}]", field_name, i);
                    if let Err(e) = validate_field(&item_field, item, inner_type) {
                        return Err(e);
                    }
                }
            } else {
                return Err(ValidationError::new(
                    field_name,
                    "array",
                    value_type_name(value),
                ));
            }
        }
        FieldType::Object(nested_schema) => {
            if let Err(errors) = validate(value, nested_schema) {
                if let Some(first) = errors.first() {
                    return Err(ValidationError::new(
                        &format!("{}.{}", field_name, first.field),
                        &first.expected,
                        &first.actual,
                    ));
                }
            }
        }
        FieldType::Optional(inner_type) => {
            if !value.is_null() {
                return validate_field(field_name, value, inner_type);
            }
        }
        FieldType::Any => {
            // Any type is always valid
        }
    }
    
    Ok(())
}

/// Get a human-readable type name for a JSON value
fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "integer"
            } else {
                "float"
            }
        }
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_validate_simple_schema() {
        let mut schema = Schema::new("user".to_string());
        schema.add_field("id".to_string(), FieldType::Integer);
        schema.add_field("name".to_string(), FieldType::String);
        schema.add_field("active".to_string(), FieldType::Boolean);
        
        // Valid data
        let valid = json!({"id": 1, "name": "Alice", "active": true});
        assert!(validate(&valid, &schema).is_ok());
        
        // Invalid type
        let invalid = json!({"id": "not-an-int", "name": "Alice", "active": true});
        assert!(validate(&invalid, &schema).is_err());
        
        // Missing field
        let missing = json!({"id": 1, "name": "Alice"});
        assert!(validate(&missing, &schema).is_err());
    }
    
    #[test]
    fn test_validate_optional_fields() {
        let mut schema = Schema::new("profile".to_string());
        schema.add_field("name".to_string(), FieldType::String);
        schema.add_field("bio".to_string(), FieldType::Optional(Box::new(FieldType::String)));
        
        // Without optional field
        let without = json!({"name": "Alice"});
        assert!(validate(&without, &schema).is_ok());
        
        // With optional field
        let with = json!({"name": "Alice", "bio": "Hello!"});
        assert!(validate(&with, &schema).is_ok());
        
        // With null optional field
        let with_null = json!({"name": "Alice", "bio": null});
        assert!(validate(&with_null, &schema).is_ok());
    }
    
    #[test]
    fn test_validate_array_field() {
        let mut schema = Schema::new("tags".to_string());
        schema.add_field("items".to_string(), FieldType::Array(Box::new(FieldType::String)));
        
        // Valid array
        let valid = json!({"items": ["a", "b", "c"]});
        assert!(validate(&valid, &schema).is_ok());
        
        // Invalid array item
        let invalid = json!({"items": ["a", 123, "c"]});
        assert!(validate(&invalid, &schema).is_err());
    }
}

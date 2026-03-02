use std::collections::{HashMap, HashSet};

use anyhow::anyhow;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, patch},
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

use crate::state::AppState;
use crate::{
    audit::write_from_headers_best_effort,
    error::{AppError, AppResult},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/field-definitions",
            get(list_field_definitions).post(create_field_definition),
        )
        .route(
            "/field-definitions/{field_id}",
            patch(update_field_definition),
        )
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(super) struct FieldDefinitionRecord {
    pub id: i64,
    pub field_key: String,
    pub name: String,
    pub field_type: String,
    pub max_length: Option<i32>,
    pub required: bool,
    pub options: Option<Value>,
    pub scanner_enabled: bool,
    pub is_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct FieldDefinitionResponse {
    id: i64,
    field_key: String,
    name: String,
    field_type: String,
    max_length: Option<i32>,
    required: bool,
    options: Option<Vec<String>>,
    scanner_enabled: bool,
    is_enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateFieldDefinitionRequest {
    field_key: String,
    name: String,
    field_type: String,
    max_length: Option<u32>,
    required: Option<bool>,
    options: Option<Vec<String>>,
    scanner_enabled: Option<bool>,
    is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct UpdateFieldDefinitionRequest {
    name: Option<String>,
    max_length: Option<Option<u32>>,
    required: Option<bool>,
    options: Option<Option<Vec<String>>>,
    scanner_enabled: Option<bool>,
    is_enabled: Option<bool>,
}

#[derive(Debug, Clone, Copy)]
enum FieldType {
    Text,
    Integer,
    Float,
    Boolean,
    Enum,
    Date,
    DateTime,
}

impl FieldType {
    fn parse(raw: &str) -> AppResult<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "integer" => Ok(Self::Integer),
            "float" => Ok(Self::Float),
            "boolean" => Ok(Self::Boolean),
            "enum" => Ok(Self::Enum),
            "date" => Ok(Self::Date),
            "datetime" => Ok(Self::DateTime),
            _ => Err(AppError::Validation(format!(
                "invalid field_type, supported values: {}",
                supported_types()
            ))),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::Boolean => "boolean",
            Self::Enum => "enum",
            Self::Date => "date",
            Self::DateTime => "datetime",
        }
    }

    fn supports_max_length(self) -> bool {
        matches!(self, Self::Text | Self::Enum)
    }
}

#[derive(Debug)]
struct NormalizedFieldDefinitionInput {
    field_key: String,
    name: String,
    field_type: FieldType,
    max_length: Option<i32>,
    required: bool,
    options_json: Option<Value>,
    scanner_enabled: bool,
    is_enabled: bool,
}

async fn list_field_definitions(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FieldDefinitionResponse>>> {
    let records: Vec<FieldDefinitionRecord> = sqlx::query_as(
        "SELECT id, field_key, name, field_type, max_length, required, options, scanner_enabled, is_enabled, created_at, updated_at
         FROM asset_field_definitions
         ORDER BY id ASC",
    )
    .fetch_all(&state.db)
    .await?;

    let mut items = Vec::with_capacity(records.len());
    for record in records {
        items.push(to_response(record)?);
    }

    Ok(Json(items))
}

async fn create_field_definition(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateFieldDefinitionRequest>,
) -> AppResult<Json<FieldDefinitionResponse>> {
    let normalized = normalize_field_definition_input(
        payload.field_key,
        payload.name,
        FieldType::parse(&payload.field_type)?,
        payload.max_length,
        payload.required.unwrap_or(false),
        payload.options,
        payload.scanner_enabled.unwrap_or(false),
        payload.is_enabled.unwrap_or(true),
    )?;

    let record: FieldDefinitionRecord = sqlx::query_as(
        "INSERT INTO asset_field_definitions (field_key, name, field_type, max_length, required, options, scanner_enabled, is_enabled)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, field_key, name, field_type, max_length, required, options, scanner_enabled, is_enabled, created_at, updated_at",
    )
    .bind(normalized.field_key)
    .bind(normalized.name)
    .bind(normalized.field_type.as_str())
    .bind(normalized.max_length)
    .bind(normalized.required)
    .bind(normalized.options_json)
    .bind(normalized.scanner_enabled)
    .bind(normalized.is_enabled)
    .fetch_one(&state.db)
    .await
    .map_err(map_field_definition_conflict)?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "cmdb.field_definition.create",
        "field_definition",
        Some(record.id.to_string()),
        "success",
        None,
        serde_json::json!({
            "field_key": &record.field_key,
            "field_type": &record.field_type
        }),
    )
    .await;

    Ok(Json(to_response(record)?))
}

async fn update_field_definition(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(field_id): Path<i64>,
    Json(payload): Json<UpdateFieldDefinitionRequest>,
) -> AppResult<Json<FieldDefinitionResponse>> {
    let existing: Option<FieldDefinitionRecord> = sqlx::query_as(
        "SELECT id, field_key, name, field_type, max_length, required, options, scanner_enabled, is_enabled, created_at, updated_at
         FROM asset_field_definitions
         WHERE id = $1",
    )
    .bind(field_id)
    .fetch_optional(&state.db)
    .await?;

    let existing = existing
        .ok_or_else(|| AppError::NotFound(format!("field definition {field_id} not found")))?;

    let field_type = FieldType::parse(&existing.field_type)?;
    let existing_options = decode_option_strings(existing.options.clone())?;
    let existing_max_length = existing.max_length.map(|value| value as u32);

    let normalized = normalize_field_definition_input(
        existing.field_key.clone(),
        payload.name.unwrap_or(existing.name.clone()),
        field_type,
        payload.max_length.unwrap_or(existing_max_length),
        payload.required.unwrap_or(existing.required),
        payload.options.unwrap_or(existing_options),
        payload.scanner_enabled.unwrap_or(existing.scanner_enabled),
        payload.is_enabled.unwrap_or(existing.is_enabled),
    )?;

    let updated: FieldDefinitionRecord = sqlx::query_as(
        "UPDATE asset_field_definitions
         SET name = $1,
             max_length = $2,
             required = $3,
             options = $4,
             scanner_enabled = $5,
             is_enabled = $6,
             updated_at = NOW()
         WHERE id = $7
         RETURNING id, field_key, name, field_type, max_length, required, options, scanner_enabled, is_enabled, created_at, updated_at",
    )
    .bind(normalized.name)
    .bind(normalized.max_length)
    .bind(normalized.required)
    .bind(normalized.options_json)
    .bind(normalized.scanner_enabled)
    .bind(normalized.is_enabled)
    .bind(field_id)
    .fetch_one(&state.db)
    .await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "cmdb.field_definition.update",
        "field_definition",
        Some(updated.id.to_string()),
        "success",
        None,
        serde_json::json!({
            "field_key": &updated.field_key,
            "field_type": &updated.field_type
        }),
    )
    .await;

    Ok(Json(to_response(updated)?))
}

pub(super) async fn fetch_enabled_definitions(
    db: &PgPool,
) -> AppResult<HashMap<String, FieldDefinitionRecord>> {
    let items: Vec<FieldDefinitionRecord> = sqlx::query_as(
        "SELECT id, field_key, name, field_type, max_length, required, options, scanner_enabled, is_enabled, created_at, updated_at
         FROM asset_field_definitions
         WHERE is_enabled = TRUE
         ORDER BY id ASC",
    )
    .fetch_all(db)
    .await?;

    Ok(items
        .into_iter()
        .map(|item| (item.field_key.clone(), item))
        .collect())
}

pub(super) fn validate_custom_field_value(
    definition: &FieldDefinitionRecord,
    value: &Value,
) -> AppResult<()> {
    let field_type = FieldType::parse(&definition.field_type)?;
    match field_type {
        FieldType::Text => {
            let text = value.as_str().ok_or_else(|| {
                AppError::Validation(format!(
                    "custom field '{}' must be a string",
                    definition.field_key
                ))
            })?;
            validate_max_length(definition, text)?;
        }
        FieldType::Integer => {
            if value.as_i64().is_none() && value.as_u64().is_none() {
                return Err(AppError::Validation(format!(
                    "custom field '{}' must be an integer",
                    definition.field_key
                )));
            }
        }
        FieldType::Float => {
            if value.as_f64().is_none() {
                return Err(AppError::Validation(format!(
                    "custom field '{}' must be a number",
                    definition.field_key
                )));
            }
        }
        FieldType::Boolean => {
            if !value.is_boolean() {
                return Err(AppError::Validation(format!(
                    "custom field '{}' must be a boolean",
                    definition.field_key
                )));
            }
        }
        FieldType::Enum => {
            let text = value.as_str().ok_or_else(|| {
                AppError::Validation(format!(
                    "custom field '{}' must be a string (enum option)",
                    definition.field_key
                ))
            })?;
            validate_max_length(definition, text)?;

            let options = decode_option_strings(definition.options.clone())?.ok_or_else(|| {
                AppError::Internal(anyhow!(
                    "field definition '{}' is enum but options missing",
                    definition.field_key
                ))
            })?;
            if !options.iter().any(|item| item == text) {
                return Err(AppError::Validation(format!(
                    "custom field '{}' must be one of: {}",
                    definition.field_key,
                    options.join(", ")
                )));
            }
        }
        FieldType::Date => {
            let text = value.as_str().ok_or_else(|| {
                AppError::Validation(format!(
                    "custom field '{}' must be a date string: YYYY-MM-DD",
                    definition.field_key
                ))
            })?;
            NaiveDate::parse_from_str(text, "%Y-%m-%d").map_err(|_| {
                AppError::Validation(format!(
                    "custom field '{}' must follow date format YYYY-MM-DD",
                    definition.field_key
                ))
            })?;
        }
        FieldType::DateTime => {
            let text = value.as_str().ok_or_else(|| {
                AppError::Validation(format!(
                    "custom field '{}' must be RFC3339 datetime string",
                    definition.field_key
                ))
            })?;
            DateTime::parse_from_rfc3339(text).map_err(|_| {
                AppError::Validation(format!(
                    "custom field '{}' must follow RFC3339 datetime format",
                    definition.field_key
                ))
            })?;
        }
    }

    Ok(())
}

fn to_response(record: FieldDefinitionRecord) -> AppResult<FieldDefinitionResponse> {
    Ok(FieldDefinitionResponse {
        id: record.id,
        field_key: record.field_key,
        name: record.name,
        field_type: record.field_type,
        max_length: record.max_length,
        required: record.required,
        options: decode_option_strings(record.options)?,
        scanner_enabled: record.scanner_enabled,
        is_enabled: record.is_enabled,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

fn normalize_field_definition_input(
    field_key: String,
    name: String,
    field_type: FieldType,
    max_length: Option<u32>,
    required: bool,
    options: Option<Vec<String>>,
    scanner_enabled: bool,
    is_enabled: bool,
) -> AppResult<NormalizedFieldDefinitionInput> {
    let normalized_field_key = normalize_field_key(field_key)?;
    let normalized_name = required_field("name", name)?;

    if max_length.is_some() && !field_type.supports_max_length() {
        return Err(AppError::Validation(
            "max_length is only supported by text/enum field types".to_string(),
        ));
    }

    let max_length_i32 = max_length.map(|value| value as i32);
    let options_json = normalize_options(field_type, options, max_length_i32)?;

    Ok(NormalizedFieldDefinitionInput {
        field_key: normalized_field_key,
        name: normalized_name,
        field_type,
        max_length: max_length_i32,
        required,
        options_json,
        scanner_enabled,
        is_enabled,
    })
}

fn normalize_field_key(field_key: String) -> AppResult<String> {
    let value = field_key.trim().to_ascii_lowercase();
    if value.is_empty() {
        return Err(AppError::Validation("field_key is required".to_string()));
    }

    let mut chars = value.chars();
    let first = chars
        .next()
        .ok_or_else(|| AppError::Validation("field_key is required".to_string()))?;

    if !first.is_ascii_lowercase() {
        return Err(AppError::Validation(
            "field_key must start with lowercase letter".to_string(),
        ));
    }

    if value.len() > 64 {
        return Err(AppError::Validation(
            "field_key length must be <= 64".to_string(),
        ));
    }

    if !value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        return Err(AppError::Validation(
            "field_key can only contain lowercase letters, numbers, and underscores".to_string(),
        ));
    }

    Ok(value)
}

fn normalize_options(
    field_type: FieldType,
    options: Option<Vec<String>>,
    max_length: Option<i32>,
) -> AppResult<Option<Value>> {
    if !matches!(field_type, FieldType::Enum) {
        if options.is_some() {
            return Err(AppError::Validation(
                "options is only supported for enum fields".to_string(),
            ));
        }
        return Ok(None);
    }

    let raw_options = options
        .ok_or_else(|| AppError::Validation("enum fields require non-empty options".to_string()))?;

    let mut normalized = Vec::with_capacity(raw_options.len());
    let mut uniq = HashSet::new();
    for option in raw_options {
        let trimmed = option.trim();
        if trimmed.is_empty() {
            return Err(AppError::Validation(
                "enum options cannot contain empty value".to_string(),
            ));
        }
        if let Some(limit) = max_length {
            if trimmed.chars().count() > limit as usize {
                return Err(AppError::Validation(format!(
                    "enum option '{}' exceeds max_length {}",
                    trimmed, limit
                )));
            }
        }
        if !uniq.insert(trimmed.to_string()) {
            return Err(AppError::Validation(format!(
                "enum options contain duplicate value '{}'",
                trimmed
            )));
        }
        normalized.push(trimmed.to_string());
    }

    if normalized.is_empty() {
        return Err(AppError::Validation(
            "enum fields require non-empty options".to_string(),
        ));
    }

    Ok(Some(Value::Array(
        normalized.into_iter().map(Value::String).collect(),
    )))
}

fn decode_option_strings(options: Option<Value>) -> AppResult<Option<Vec<String>>> {
    let Some(value) = options else {
        return Ok(None);
    };

    let array = value.as_array().ok_or_else(|| {
        AppError::Internal(anyhow!("field definition options must be a JSON array"))
    })?;

    let mut items = Vec::with_capacity(array.len());
    for item in array {
        let option = item.as_str().ok_or_else(|| {
            AppError::Internal(anyhow!("field definition options must contain strings"))
        })?;
        items.push(option.to_string());
    }

    Ok(Some(items))
}

fn validate_max_length(definition: &FieldDefinitionRecord, text: &str) -> AppResult<()> {
    if let Some(limit) = definition.max_length {
        if text.chars().count() > limit as usize {
            return Err(AppError::Validation(format!(
                "custom field '{}' exceeds max_length {}",
                definition.field_key, limit
            )));
        }
    }
    Ok(())
}

fn required_field(field: &str, value: String) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(format!("{field} is required")));
    }
    Ok(trimmed.to_string())
}

fn supported_types() -> &'static str {
    "text, integer, float, boolean, enum, date, datetime"
}

fn map_field_definition_conflict(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some("23505") {
            return AppError::Validation("field_key already exists".to_string());
        }
    }
    AppError::Database(err)
}

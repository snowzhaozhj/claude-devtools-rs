//! Trigger CRUD + 校验 + merge。
//!
//! 对应 TS `TriggerManager.ts`。只负责数据管理和校验，
//! trigger 评估逻辑留给 `port-notification-triggers`。

use crate::error::ConfigError;
use crate::regex_safety::validate_regex_pattern;
use crate::types::{NotificationTrigger, TriggerMode};

/// Trigger 校验结果。
#[derive(Debug, Clone)]
pub struct TriggerValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

/// 校验单个 trigger 配置。
pub fn validate_trigger(trigger: &NotificationTrigger) -> TriggerValidationResult {
    let mut errors = Vec::new();

    if trigger.id.trim().is_empty() {
        errors.push("Trigger ID is required".into());
    }
    if trigger.name.trim().is_empty() {
        errors.push("Trigger name is required".into());
    }

    // mode 特异校验
    match trigger.mode {
        TriggerMode::ContentMatch => {
            // `matchField` 在非 tool_use + 无 toolName 时必填
            let is_any_tool_use = matches!(
                trigger.content_type,
                crate::types::TriggerContentType::ToolUse
            ) && trigger.tool_name.is_none();
            if trigger.match_field.is_none() && !is_any_tool_use {
                errors.push("Match field is required for content_match mode".into());
            }
            if let Some(ref pat) = trigger.match_pattern {
                let v = validate_regex_pattern(pat);
                if !v.valid {
                    errors.push(v.error.unwrap_or_else(|| "Invalid regex pattern".into()));
                }
            }
        }
        TriggerMode::TokenThreshold => {
            if trigger.token_threshold.is_none() {
                errors.push("Token threshold must be a non-negative number".into());
            }
            if trigger.token_type.is_none() {
                errors.push("Token type is required for token_threshold mode".into());
            }
        }
        TriggerMode::ErrorStatus => {}
    }

    // 校验 ignore patterns
    if let Some(ref patterns) = trigger.ignore_patterns {
        for pat in patterns {
            let v = validate_regex_pattern(pat);
            if !v.valid {
                errors.push(format!(
                    "Invalid ignore pattern \"{pat}\": {}",
                    v.error.unwrap_or_else(|| "Unknown error".into())
                ));
            }
        }
    }

    TriggerValidationResult {
        valid: errors.is_empty(),
        errors,
    }
}

/// Trigger 管理器。管理 trigger 列表的 CRUD 操作。
#[derive(Debug, Clone)]
pub struct TriggerManager {
    triggers: Vec<NotificationTrigger>,
}

impl TriggerManager {
    pub fn new(triggers: Vec<NotificationTrigger>) -> Self {
        Self { triggers }
    }

    pub fn get_all(&self) -> Vec<NotificationTrigger> {
        self.triggers.clone()
    }

    pub fn get_enabled(&self) -> Vec<NotificationTrigger> {
        self.triggers
            .iter()
            .filter(|t| t.enabled)
            .cloned()
            .collect()
    }

    /// 添加 trigger，返回更新后的列表。
    pub fn add(
        &mut self,
        trigger: NotificationTrigger,
    ) -> Result<Vec<NotificationTrigger>, ConfigError> {
        if self.triggers.iter().any(|t| t.id == trigger.id) {
            return Err(ConfigError::validation(format!(
                "Trigger with ID \"{}\" already exists",
                trigger.id
            )));
        }

        let validation = validate_trigger(&trigger);
        if !validation.valid {
            return Err(ConfigError::validation(format!(
                "Invalid trigger: {}",
                validation.errors.join(", ")
            )));
        }

        self.triggers.push(trigger);
        Ok(self.get_all())
    }

    /// 更新 trigger，`is_builtin` 不可修改。
    pub fn update(
        &mut self,
        trigger_id: &str,
        updates: &serde_json::Value,
    ) -> Result<Vec<NotificationTrigger>, ConfigError> {
        let idx = self
            .triggers
            .iter()
            .position(|t| t.id == trigger_id)
            .ok_or_else(|| {
                ConfigError::validation(format!("Trigger with ID \"{trigger_id}\" not found"))
            })?;

        // 序列化当前 trigger → 合并 updates → 反序列化
        let mut current = serde_json::to_value(&self.triggers[idx])
            .map_err(|e| ConfigError::validation(format!("Failed to serialize trigger: {e}")))?;

        if let (Some(cur_map), Some(upd_map)) = (current.as_object_mut(), updates.as_object()) {
            for (k, v) in upd_map {
                if k == "isBuiltin" {
                    continue; // 不允许修改
                }
                cur_map.insert(k.clone(), v.clone());
            }
        }

        let updated: NotificationTrigger = serde_json::from_value(current)
            .map_err(|e| ConfigError::validation(format!("Invalid trigger update: {e}")))?;

        let validation = validate_trigger(&updated);
        if !validation.valid {
            return Err(ConfigError::validation(format!(
                "Invalid trigger update: {}",
                validation.errors.join(", ")
            )));
        }

        self.triggers[idx] = updated;
        Ok(self.get_all())
    }

    /// 删除 trigger。内建 trigger 不可删除。
    pub fn remove(&mut self, trigger_id: &str) -> Result<Vec<NotificationTrigger>, ConfigError> {
        let trigger = self
            .triggers
            .iter()
            .find(|t| t.id == trigger_id)
            .ok_or_else(|| {
                ConfigError::validation(format!("Trigger with ID \"{trigger_id}\" not found"))
            })?;

        if trigger.is_builtin() {
            return Err(ConfigError::validation(
                "Cannot remove built-in triggers. Disable them instead.",
            ));
        }

        self.triggers.retain(|t| t.id != trigger_id);
        Ok(self.get_all())
    }

    /// 替换整个 trigger 列表（用于 reload 时）。
    pub fn set_triggers(&mut self, triggers: Vec<NotificationTrigger>) {
        self.triggers = triggers;
    }
}

/// 合并 trigger：保留已有 + 补齐缺失 builtin + 移除过期 builtin。
pub fn merge_triggers(
    loaded: &[NotificationTrigger],
    defaults: &[NotificationTrigger],
) -> Vec<NotificationTrigger> {
    let builtin_ids: std::collections::HashSet<&str> = defaults
        .iter()
        .filter(|t| t.is_builtin())
        .map(|t| t.id.as_str())
        .collect();

    // 移除已不在 defaults 中的过期 builtin
    let mut merged: Vec<NotificationTrigger> = loaded
        .iter()
        .filter(|t| !t.is_builtin() || builtin_ids.contains(t.id.as_str()))
        .cloned()
        .collect();

    // 补齐缺失的 builtin
    for dt in defaults {
        if dt.is_builtin() && !merged.iter().any(|t| t.id == dt.id) {
            merged.push(dt.clone());
        }
    }

    merged
}

/// 从当前 trigger 的属性推断 mode（向后兼容）。
pub fn infer_mode(trigger: &NotificationTrigger) -> TriggerMode {
    if trigger.require_error == Some(true) {
        return TriggerMode::ErrorStatus;
    }
    if trigger.match_pattern.is_some() || trigger.match_field.is_some() {
        return TriggerMode::ContentMatch;
    }
    if trigger.token_threshold.is_some() {
        return TriggerMode::TokenThreshold;
    }
    TriggerMode::ErrorStatus
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults::default_triggers;
    use crate::types::TriggerContentType;

    fn make_trigger(id: &str, builtin: bool) -> NotificationTrigger {
        NotificationTrigger {
            id: id.into(),
            name: format!("Test {id}"),
            enabled: true,
            content_type: TriggerContentType::ToolResult,
            mode: TriggerMode::ErrorStatus,
            require_error: Some(true),
            is_builtin: Some(builtin),
            tool_name: None,
            ignore_patterns: None,
            match_field: None,
            match_pattern: None,
            token_threshold: None,
            token_type: None,
            repository_ids: None,
            color: None,
        }
    }

    #[test]
    fn add_trigger_success() {
        let mut mgr = TriggerManager::new(vec![]);
        let t = make_trigger("custom-1", false);
        let result = mgr.add(t);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn add_duplicate_trigger_fails() {
        let mut mgr = TriggerManager::new(vec![make_trigger("t1", false)]);
        let t = make_trigger("t1", false);
        assert!(mgr.add(t).is_err());
    }

    #[test]
    fn remove_builtin_fails() {
        let mut mgr = TriggerManager::new(vec![make_trigger("b1", true)]);
        let result = mgr.remove("b1");
        assert!(result.is_err());
    }

    #[test]
    fn remove_custom_success() {
        let mut mgr = TriggerManager::new(vec![make_trigger("c1", false)]);
        let result = mgr.remove("c1");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn merge_adds_missing_builtin() {
        let loaded = vec![make_trigger("custom", false)];
        let defaults = default_triggers();
        let merged = merge_triggers(&loaded, &defaults);

        // custom + 3 builtins
        assert_eq!(merged.len(), 4);
        assert!(merged.iter().any(|t| t.id == "builtin-bash-command"));
    }

    #[test]
    fn merge_removes_stale_builtin() {
        let old_builtin = make_trigger("deprecated-builtin", true);
        let loaded = vec![old_builtin, make_trigger("custom", false)];
        let defaults = default_triggers();
        let merged = merge_triggers(&loaded, &defaults);

        // `deprecated-builtin` 不在 defaults 中，应被移除
        assert!(!merged.iter().any(|t| t.id == "deprecated-builtin"));
        assert!(merged.iter().any(|t| t.id == "custom"));
    }

    #[test]
    fn validate_content_match_without_field() {
        let mut t = make_trigger("t1", false);
        t.mode = TriggerMode::ContentMatch;
        t.match_field = None;
        t.require_error = None;
        t.content_type = TriggerContentType::ToolResult;
        let v = validate_trigger(&t);
        assert!(!v.valid);
        assert!(v.errors.iter().any(|e| e.contains("Match field")));
    }

    #[test]
    fn validate_token_threshold_missing() {
        let mut t = make_trigger("t1", false);
        t.mode = TriggerMode::TokenThreshold;
        t.token_threshold = None;
        t.token_type = None;
        t.require_error = None;
        let v = validate_trigger(&t);
        assert!(!v.valid);
        assert_eq!(v.errors.len(), 2);
    }

    #[test]
    fn get_enabled_filters() {
        let mut t1 = make_trigger("t1", false);
        t1.enabled = true;
        let mut t2 = make_trigger("t2", false);
        t2.enabled = false;
        let mgr = TriggerManager::new(vec![t1, t2]);
        assert_eq!(mgr.get_enabled().len(), 1);
    }

    #[test]
    fn infer_mode_from_properties() {
        let mut t = make_trigger("t", false);
        t.require_error = Some(true);
        assert_eq!(infer_mode(&t), TriggerMode::ErrorStatus);

        t.require_error = None;
        t.match_pattern = Some("test".into());
        assert_eq!(infer_mode(&t), TriggerMode::ContentMatch);

        t.match_pattern = None;
        t.token_threshold = Some(1000);
        assert_eq!(infer_mode(&t), TriggerMode::TokenThreshold);
    }
}

//! Reusable Yazi asset metadata and deterministic config-pack rendering.
//!
//! This crate is intentionally pure: it renders TOML/Lua content from explicit
//! inputs and never reads user config paths, generated state directories, or
//! host environment variables.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use toml::Value as TomlValue;

mod starship;

pub use starship::render_yazelix_starship_config;

const APPEARANCE_MODE_DARK: &str = "dark";
const APPEARANCE_MODE_LIGHT: &str = "light";
const YAZI_THEME_LIGHT: &str = "catppuccin-latte";
const RUNTIME_DIR_PLACEHOLDER: &str = "__YAZELIX_RUNTIME_DIR__";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YaziConfigPackError {
    InvalidSortBy {
        sort_by: String,
        allowed: Vec<String>,
    },
    InvalidEmbeddedToml {
        asset: &'static str,
        message: String,
    },
    SerializeToml {
        message: String,
    },
}

impl fmt::Display for YaziConfigPackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSortBy { sort_by, allowed } => {
                write!(
                    f,
                    "yazi.sort_by must be one of {allowed:?} (got {sort_by:?})"
                )
            }
            Self::InvalidEmbeddedToml { asset, message } => {
                write!(f, "could not parse embedded {asset}: {message}")
            }
            Self::SerializeToml { message } => {
                write!(f, "could not serialize generated Yazi TOML: {message}")
            }
        }
    }
}

impl std::error::Error for YaziConfigPackError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct YaziRenderPlanMetadata {
    sort_by_allowed: Vec<String>,
    default_plugins: Vec<String>,
    themes_dark: Vec<String>,
    themes_light: Vec<String>,
    core_plugins: Vec<String>,
}

static YAZI_RENDER_PLAN_METADATA: OnceLock<YaziRenderPlanMetadata> = OnceLock::new();

fn yazi_render_plan_metadata() -> &'static YaziRenderPlanMetadata {
    YAZI_RENDER_PLAN_METADATA.get_or_init(|| {
        toml::from_str(include_str!("../config_metadata/yazi_render_plan.toml"))
            .expect("embedded config_metadata/yazi_render_plan.toml must parse")
    })
}

fn default_yazi_theme() -> String {
    "default".into()
}

fn default_yazi_sort_by() -> String {
    "alphabetical".into()
}

fn default_yazi_plugins() -> Vec<String> {
    yazi_render_plan_metadata().default_plugins.clone()
}

fn default_appearance_mode() -> String {
    APPEARANCE_MODE_DARK.into()
}

fn pick_index(len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| (d.as_nanos() as usize) % len)
        .unwrap_or(0)
}

fn appearance_default_theme(configured_theme: &str, light_theme: &str, mode: &str) -> String {
    if mode == APPEARANCE_MODE_LIGHT && configured_theme == "default" {
        light_theme.to_string()
    } else {
        configured_theme.to_string()
    }
}

fn resolve_yazi_theme(theme_config: &str) -> String {
    let meta = yazi_render_plan_metadata();
    match theme_config {
        "random-dark" => meta
            .themes_dark
            .get(pick_index(meta.themes_dark.len()))
            .cloned()
            .unwrap_or_else(|| "default".into()),
        "random-light" => meta
            .themes_light
            .get(pick_index(meta.themes_light.len()))
            .cloned()
            .unwrap_or_else(|| "default".into()),
        _ => theme_config.to_string(),
    }
}

fn validate_sort_by(sort_by: &str) -> Result<(), YaziConfigPackError> {
    let allowed = &yazi_render_plan_metadata().sort_by_allowed;
    if allowed.iter().any(|v| v == sort_by) {
        Ok(())
    } else {
        Err(YaziConfigPackError::InvalidSortBy {
            sort_by: sort_by.to_string(),
            allowed: allowed.clone(),
        })
    }
}

fn merged_plugin_load_order(user_plugins: &[String]) -> Vec<String> {
    let meta = yazi_render_plan_metadata();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for p in meta
        .core_plugins
        .iter()
        .cloned()
        .chain(user_plugins.iter().cloned())
    {
        if seen.insert(p.clone()) {
            out.push(p);
        }
    }
    out
}

fn theme_flavor_plan(resolved_theme: &str) -> ThemeFlavorPlan {
    if resolved_theme == "default" || resolved_theme == "random" {
        ThemeFlavorPlan::None
    } else {
        ThemeFlavorPlan::Uniform {
            flavor: resolved_theme.to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YaziRenderPlanRequest {
    #[serde(default = "default_yazi_theme")]
    pub yazi_theme: String,
    #[serde(default = "default_appearance_mode")]
    pub appearance_mode: String,
    #[serde(default = "default_yazi_sort_by")]
    pub yazi_sort_by: String,
    #[serde(default)]
    pub yazi_plugins: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ThemeFlavorPlan {
    None,
    Uniform { flavor: String },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InitLuaPlan {
    pub core_plugins: Vec<String>,
    pub load_order: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct YaziRenderPlanData {
    pub resolved_theme: String,
    pub sort_by: String,
    pub yazi_plugins: Vec<String>,
    pub git_plugin_enabled: bool,
    pub theme_flavor: ThemeFlavorPlan,
    pub init_lua: InitLuaPlan,
}

pub fn compute_yazi_render_plan(
    request: &YaziRenderPlanRequest,
) -> Result<YaziRenderPlanData, YaziConfigPackError> {
    validate_sort_by(&request.yazi_sort_by)?;

    let yazi_plugins = request
        .yazi_plugins
        .clone()
        .unwrap_or_else(default_yazi_plugins);
    let git_plugin_enabled = yazi_plugins.iter().any(|p| p == "git");
    let theme_config = appearance_default_theme(
        &request.yazi_theme,
        YAZI_THEME_LIGHT,
        &request.appearance_mode,
    );
    let resolved_theme = resolve_yazi_theme(&theme_config);
    let theme_flavor = theme_flavor_plan(&resolved_theme);
    let load_order = merged_plugin_load_order(&yazi_plugins);
    let core_plugins = yazi_render_plan_metadata().core_plugins.clone();

    Ok(YaziRenderPlanData {
        resolved_theme,
        sort_by: request.yazi_sort_by.clone(),
        yazi_plugins,
        git_plugin_enabled,
        theme_flavor,
        init_lua: InitLuaPlan {
            core_plugins,
            load_order,
        },
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct YaziConfigPackTemplates {
    pub yazi_toml: toml::Table,
    pub theme_toml: toml::Table,
    pub keymap_toml: toml::Table,
}

impl YaziConfigPackTemplates {
    pub fn bundled() -> Result<Self, YaziConfigPackError> {
        Ok(Self {
            yazi_toml: parse_embedded_toml_table(
                "config_templates/yazelix_yazi.toml",
                include_str!("../config_templates/yazelix_yazi.toml"),
            )?,
            theme_toml: parse_embedded_toml_table(
                "config_templates/yazelix_theme.toml",
                include_str!("../config_templates/yazelix_theme.toml"),
            )?,
            keymap_toml: parse_embedded_toml_table(
                "config_templates/yazelix_keymap.toml",
                include_str!("../config_templates/yazelix_keymap.toml"),
            )?,
        })
    }
}

#[derive(Debug)]
pub struct YaziConfigPackRenderRequest<'a> {
    pub templates: &'a YaziConfigPackTemplates,
    pub runtime_dir: &'a str,
    pub starship_config_path: &'a str,
    pub render_plan: &'a YaziRenderPlanData,
    pub user_yazi_config: Option<&'a toml::Table>,
    pub user_keymap: Option<&'a toml::Table>,
    pub user_init_lua: Option<&'a str>,
    pub semantic_keymap: &'a toml::Table,
    pub available_plugins: &'a BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YaziConfigPackRenderOutput {
    pub yazi_toml: String,
    pub theme_toml: String,
    pub keymap_toml: String,
    pub init_lua: String,
    pub missing_plugins: Vec<String>,
    pub user_init_appended: bool,
}

pub fn render_yazi_config_pack(
    request: &YaziConfigPackRenderRequest<'_>,
) -> Result<YaziConfigPackRenderOutput, YaziConfigPackError> {
    let yazi_toml = render_generated_yazi_toml(request)?;
    let theme_toml = render_generated_theme_toml(request)?;
    let keymap_toml = render_generated_keymap_toml(request)?;
    let (init_lua, missing_plugins, user_init_appended) = render_generated_init_lua(request);

    Ok(YaziConfigPackRenderOutput {
        yazi_toml,
        theme_toml,
        keymap_toml,
        init_lua,
        missing_plugins,
        user_init_appended,
    })
}

fn render_generated_yazi_toml(
    request: &YaziConfigPackRenderRequest<'_>,
) -> Result<String, YaziConfigPackError> {
    let mut final_config = if let Some(user_config) = request.user_yazi_config {
        merge_yazi_toml_config(request.templates.yazi_toml.clone(), user_config.clone())
    } else {
        request.templates.yazi_toml.clone()
    };

    preserve_yazelix_edit_opener(&request.templates.yazi_toml, &mut final_config);
    if !request.render_plan.git_plugin_enabled {
        final_config.remove("plugin");
    }
    upsert_nested_string(
        &mut final_config,
        &["manager"],
        "sort_by",
        &request.render_plan.sort_by,
    );

    let user_note = if request.user_yazi_config.is_some() {
        "User config merged from:"
    } else {
        "To add custom settings, create:"
    };
    let header = generated_header(
        "#",
        "AUTO-GENERATED YAZI CONFIG",
        &[
            "",
            user_note,
            "  ~/.config/yazelix/yazi/yazi.toml",
            "Dynamic settings from ~/.config/yazelix/settings.jsonc:",
            "  [yazi] sort_by, plugins",
            "",
        ],
    );
    let config_content = render_runtime_root_placeholders(
        &toml_to_string_pretty(&TomlValue::Table(final_config))?,
        request.runtime_dir,
    );
    Ok(format!("{header}{config_content}"))
}

fn render_generated_theme_toml(
    request: &YaziConfigPackRenderRequest<'_>,
) -> Result<String, YaziConfigPackError> {
    let mut base_theme = request.templates.theme_toml.clone();
    if let ThemeFlavorPlan::Uniform { flavor } = &request.render_plan.theme_flavor {
        let mut flavor_table = toml::Table::new();
        flavor_table.insert("dark".into(), TomlValue::String(flavor.clone()));
        flavor_table.insert("light".into(), TomlValue::String(flavor.clone()));
        base_theme.insert("flavor".into(), TomlValue::Table(flavor_table));
    }

    let current_theme = format!("Current theme: {}", request.render_plan.resolved_theme);
    let header = generated_header(
        "#",
        "AUTO-GENERATED YAZI THEME CONFIG",
        &[
            "",
            "To customize theme, edit:",
            "  ~/.config/yazelix/settings.jsonc",
            "  [yazi] theme = \"...\"",
            "",
            current_theme.as_str(),
        ],
    );

    let config_content = if base_theme.is_empty() {
        String::new()
    } else {
        toml_to_string_pretty(&TomlValue::Table(base_theme))?
    };
    Ok(format!("{header}{config_content}"))
}

fn render_generated_keymap_toml(
    request: &YaziConfigPackRenderRequest<'_>,
) -> Result<String, YaziConfigPackError> {
    let mut base_keymap = request.templates.keymap_toml.clone();
    base_keymap = merge_yazi_keymap(base_keymap, request.semantic_keymap.clone());

    let final_keymap = if let Some(user_keymap) = request.user_keymap {
        merge_yazi_keymap(base_keymap, user_keymap.clone())
    } else {
        base_keymap
    };

    let header = generated_header(
        "#",
        "AUTO-GENERATED YAZI KEYMAP",
        &[
            "",
            "To add custom keybindings, create:",
            "  ~/.config/yazelix/yazi/keymap.toml",
            "",
        ],
    );
    let keymap_content = render_runtime_root_placeholders(
        &toml_to_string_pretty(&TomlValue::Table(final_keymap))?,
        request.runtime_dir,
    );
    Ok(format!("{header}{keymap_content}"))
}

fn render_generated_init_lua(
    request: &YaziConfigPackRenderRequest<'_>,
) -> (String, Vec<String>, bool) {
    let core_plugins = &request.render_plan.init_lua.core_plugins;
    let all_plugins = &request.render_plan.init_lua.load_order;
    let (valid_plugins, missing_plugins): (Vec<_>, Vec<_>) = all_plugins
        .iter()
        .cloned()
        .partition(|name| request.available_plugins.contains(name));

    let requires = valid_plugins
        .iter()
        .map(|name| {
            if core_plugins.contains(name) {
                format!("-- Core plugin (always loaded)\nrequire(\"{name}\"):setup()")
            } else if name == "starship" {
                format!(
                    "-- User plugin (from settings.jsonc)\nrequire(\"starship\"):setup({{\n    config_file = \"{}\"\n}})",
                    request.starship_config_path
                )
            } else {
                let local_name = name.replace('-', "_");
                format!(
                    "-- User plugin (from settings.jsonc)\nlocal _{local_name} = require(\"{name}\")\nif type(_{local_name}.setup) == \"function\" then _{local_name}:setup() end"
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let header = generated_header(
        "--",
        "AUTO-GENERATED YAZI INIT.LUA",
        &[
            "",
            "To customize plugins, edit:",
            "  ~/.config/yazelix/settings.jsonc",
            "  [yazi] plugins = [...]",
            "",
            "For custom Lua code, create:",
            "  ~/.config/yazelix/yazi/init.lua",
            "",
        ],
    );
    let mut final_content = format!("{header}{requires}\n");
    let user_init_appended = request.user_init_lua.is_some();
    if let Some(user_init) = request.user_init_lua {
        let user_section = [
            "",
            "-- ========================================",
            "-- USER CUSTOM CODE",
            "-- ========================================",
            "-- From: ~/.config/yazelix/yazi/init.lua",
            "-- ========================================",
            "",
            user_init,
        ]
        .join("\n");
        final_content.push_str(&user_section);
    }

    (
        render_runtime_root_placeholders(&final_content, request.runtime_dir),
        missing_plugins,
        user_init_appended,
    )
}

fn generated_header(comment: &str, title: &str, body: &[&str]) -> String {
    let border = format!("{comment} ========================================");
    let mut lines = vec![
        border.clone(),
        format!("{comment} {title}"),
        border.clone(),
        format!("{comment} This file is automatically generated by Yazelix."),
        format!("{comment} Do not edit directly - changes will be lost!"),
    ];
    lines.extend(body.iter().map(|line| {
        if line.is_empty() {
            comment.to_string()
        } else {
            format!("{comment} {line}")
        }
    }));
    lines.push(border);
    lines.push(String::new());
    lines.join("\n")
}

fn upsert_nested_string(root: &mut toml::Table, path: &[&str], leaf: &str, value: &str) {
    let mut current = root;
    for segment in path {
        if !current.contains_key(*segment) {
            current.insert((*segment).into(), TomlValue::Table(toml::Table::new()));
        }
        current = current
            .get_mut(*segment)
            .and_then(TomlValue::as_table_mut)
            .expect("path inserted as nested tables");
    }
    current.insert(leaf.into(), TomlValue::String(value.to_string()));
}

fn merge_yazi_toml_config(base_config: toml::Table, user_config: toml::Table) -> toml::Table {
    let mut merged = TomlValue::Table(base_config);
    deep_merge_toml(&mut merged, &TomlValue::Table(user_config));
    merged.as_table().cloned().unwrap_or_default()
}

fn deep_merge_toml(base: &mut TomlValue, user: &TomlValue) {
    match (base, user) {
        (TomlValue::Table(base_table), TomlValue::Table(user_table)) => {
            for (key, user_value) in user_table {
                match base_table.get_mut(key) {
                    Some(base_value) => deep_merge_toml(base_value, user_value),
                    None => {
                        base_table.insert(key.clone(), user_value.clone());
                    }
                }
            }
        }
        (base_value, user_value) => {
            *base_value = user_value.clone();
        }
    }
}

fn merge_yazi_keymap(base_keymap: toml::Table, user_keymap: toml::Table) -> toml::Table {
    let mut merged = base_keymap;
    for (section, user_value) in user_keymap {
        let TomlValue::Table(user_section) = user_value else {
            merged.insert(section, user_value);
            continue;
        };

        let Some(base_section) = merged.get_mut(&section).and_then(TomlValue::as_table_mut) else {
            merged.insert(section, TomlValue::Table(user_section));
            continue;
        };

        let base_subsections = base_section.keys().cloned().collect::<Vec<_>>();
        for subsection in &base_subsections {
            let Some(user_value) = user_section.get(subsection) else {
                continue;
            };
            let Some(base_array) = base_section
                .get_mut(subsection)
                .and_then(TomlValue::as_array_mut)
            else {
                continue;
            };
            match user_value {
                TomlValue::Array(user_array) => base_array.extend(user_array.iter().cloned()),
                other => base_array.push(other.clone()),
            }
        }
        for (subsection, user_value) in user_section {
            if !base_subsections.contains(&subsection) {
                base_section.insert(subsection.clone(), user_value.clone());
            }
        }
    }
    merged
}

fn parse_embedded_toml_table(
    asset: &'static str,
    raw: &str,
) -> Result<toml::Table, YaziConfigPackError> {
    toml::from_str::<toml::Table>(raw).map_err(|source| YaziConfigPackError::InvalidEmbeddedToml {
        asset,
        message: source.to_string(),
    })
}

fn toml_to_string_pretty(value: &TomlValue) -> Result<String, YaziConfigPackError> {
    toml::to_string_pretty(value).map_err(|source| YaziConfigPackError::SerializeToml {
        message: source.to_string(),
    })
}

pub fn render_runtime_root_placeholders(content: &str, runtime_dir: &str) -> String {
    content.replace(RUNTIME_DIR_PLACEHOLDER, runtime_dir)
}

pub fn preserve_yazelix_edit_opener(base: &toml::Table, merged: &mut toml::Table) {
    let Some(base_opener) = base.get("opener").and_then(TomlValue::as_table) else {
        return;
    };
    let Some(yazelix_edit) = base_opener.get("edit").cloned() else {
        return;
    };

    if !merged.contains_key("opener") {
        merged.insert("opener".into(), TomlValue::Table(toml::Table::new()));
    }
    let opener = merged
        .get_mut("opener")
        .and_then(TomlValue::as_table_mut)
        .expect("opener inserted as a table");
    opener.insert("edit".into(), yazelix_edit);
}

// Test lane: maintainer
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> YaziRenderPlanRequest {
        YaziRenderPlanRequest {
            yazi_theme: "default".into(),
            appearance_mode: APPEARANCE_MODE_DARK.into(),
            yazi_sort_by: "alphabetical".into(),
            yazi_plugins: None,
        }
    }

    // Defends: invalid yazi.sort_by values fail before they enter generated TOML.
    #[test]
    fn rejects_invalid_sort_by() {
        let mut req = sample_request();
        req.yazi_sort_by = "not-a-sort".into();
        assert!(matches!(
            compute_yazi_render_plan(&req),
            Err(YaziConfigPackError::InvalidSortBy { .. })
        ));
    }

    // Defends: light appearance changes only the implicit default Yazi theme.
    #[test]
    fn light_appearance_changes_default_theme_only() {
        let mut req = sample_request();
        req.appearance_mode = "light".into();
        let plan = compute_yazi_render_plan(&req).unwrap();
        assert_eq!(plan.resolved_theme, "catppuccin-latte");

        req.yazi_theme = "dracula".into();
        let plan = compute_yazi_render_plan(&req).unwrap();
        assert_eq!(plan.resolved_theme, "dracula");
    }

    // Defends: init.lua load order prepends core plugins and dedupes user entries in first-wins order.
    #[test]
    fn init_load_order_merges_core_then_user_deduped() {
        let mut req = sample_request();
        req.yazi_plugins = Some(vec![
            "git".into(),
            "sidebar-status".into(),
            "starship".into(),
        ]);
        let plan = compute_yazi_render_plan(&req).unwrap();
        assert_eq!(
            plan.init_lua.load_order,
            vec![
                "sidebar-status".to_string(),
                "auto-layout".to_string(),
                "sidebar-state".to_string(),
                "git".to_string(),
                "starship".to_string(),
            ]
        );
    }

    // Defends: the child config-pack renderer preserves generated TOML/Lua behavior from explicit inputs.
    #[test]
    fn render_yazi_config_pack_preserves_generated_contracts() {
        let runtime_dir = "/runtime/yazelix";
        let starship_config_path = "/state/configs/yazi/yazelix_starship.toml";
        let render_plan = compute_yazi_render_plan(&YaziRenderPlanRequest {
            yazi_theme: "dracula".to_string(),
            appearance_mode: "dark".to_string(),
            yazi_sort_by: "modified".to_string(),
            yazi_plugins: Some(vec![
                "git".to_string(),
                "starship".to_string(),
                "missing-plugin".to_string(),
            ]),
        })
        .unwrap();
        let available_plugins = render_plan
            .init_lua
            .load_order
            .iter()
            .filter(|name| *name != "missing-plugin")
            .cloned()
            .collect::<BTreeSet<_>>();
        let user_yazi_config = toml::from_str::<toml::Table>(
            r#"
[opener]
edit = [{ run = "user-edit" }]

[mgr]
ratio = [1, 4, 0]
"#,
        )
        .unwrap();
        let semantic_keymap = toml::from_str::<toml::Table>(
            r#"[[mgr.append_keymap]]
on = ["<A-x>"]
run = "plugin zoxide-editor"
desc = "Open zoxide in editor"
"#,
        )
        .unwrap();
        let rendered = render_yazi_config_pack(&YaziConfigPackRenderRequest {
            templates: &YaziConfigPackTemplates::bundled().unwrap(),
            runtime_dir,
            starship_config_path,
            render_plan: &render_plan,
            user_yazi_config: Some(&user_yazi_config),
            user_keymap: None,
            user_init_lua: Some("-- user init\nreturn true\n"),
            semantic_keymap: &semantic_keymap,
            available_plugins: &available_plugins,
        })
        .unwrap();
        let parsed_yazi = toml::from_str::<TomlValue>(&rendered.yazi_toml).unwrap();
        let parsed_theme = toml::from_str::<TomlValue>(&rendered.theme_toml).unwrap();
        let parsed_keymap = toml::from_str::<TomlValue>(&rendered.keymap_toml).unwrap();

        assert_eq!(
            parsed_yazi
                .get("opener")
                .and_then(TomlValue::as_table)
                .and_then(|opener| opener.get("edit"))
                .and_then(TomlValue::as_array)
                .and_then(|entries| entries.first())
                .and_then(|entry| entry.get("run"))
                .and_then(TomlValue::as_str),
            Some("/runtime/yazelix/libexec/yzx_control zellij open-editor %s")
        );
        assert_eq!(
            parsed_yazi
                .get("mgr")
                .and_then(TomlValue::as_table)
                .and_then(|mgr| mgr.get("ratio"))
                .and_then(TomlValue::as_array)
                .unwrap(),
            &vec![1.into(), 4.into(), 0.into()]
        );
        assert_eq!(
            parsed_yazi
                .get("manager")
                .and_then(TomlValue::as_table)
                .and_then(|mgr| mgr.get("sort_by"))
                .and_then(TomlValue::as_str),
            Some("modified")
        );
        assert_eq!(
            parsed_keymap
                .get("mgr")
                .and_then(|section| section.get("append_keymap"))
                .and_then(TomlValue::as_array)
                .and_then(|entries| entries.first())
                .and_then(|entry| entry.get("run"))
                .and_then(TomlValue::as_str),
            Some("plugin zoxide-editor")
        );
        assert_eq!(rendered.missing_plugins, vec!["missing-plugin"]);
        assert!(rendered.init_lua.contains("require(\"starship\")"));
        assert!(rendered.init_lua.contains(starship_config_path));
        assert!(rendered.init_lua.contains("-- user init"));
        assert!(!rendered.yazi_toml.contains(RUNTIME_DIR_PLACEHOLDER));

        let icon = parsed_theme
            .get("icon")
            .and_then(TomlValue::as_table)
            .expect("generated theme icon table");
        let prepend_files = icon
            .get("prepend_files")
            .and_then(TomlValue::as_array)
            .expect("generated file icon overrides");
        let prepend_exts = icon
            .get("prepend_exts")
            .and_then(TomlValue::as_array)
            .expect("generated extension icon overrides");
        assert!(has_icon(prepend_files, "README.md", "MD"));
        assert!(has_icon(prepend_files, "robots.txt", "T"));
        assert!(has_icon(prepend_files, "sitemap.xml", "<>"));
        assert!(has_icon(prepend_exts, "md", "MD"));
        assert!(has_icon(prepend_exts, "txt", "T"));
        assert!(has_icon(prepend_exts, "xml", "<>"));
    }

    fn has_icon(entries: &[TomlValue], name: &str, text: &str) -> bool {
        entries.iter().any(|entry| {
            entry.get("name").and_then(TomlValue::as_str) == Some(name)
                && entry.get("text").and_then(TomlValue::as_str) == Some(text)
        })
    }

    // Defends: the active bundled flavor carries the same rendered-safe document icons
    // used by the generated theme sidecar.
    #[test]
    fn neon_flavor_uses_rendered_safe_document_icons() {
        let raw = include_str!("../flavors/neon.yazi/flavor.toml");

        for expected in [
            r#"{ url = "README*", text = "MD" }"#,
            r#"{ url = "*.md", text = "MD" }"#,
            r#"{ url = "robots.txt", text = "T" }"#,
            r#"{ url = "*.txt", text = "T" }"#,
            r#"{ url = "sitemap.xml", text = "<>" }"#,
            r#"{ url = "*.xml", text = "<>" }"#,
        ] {
            assert!(
                raw.contains(expected),
                "missing rendered-safe neon icon glob: {expected}"
            );
        }
    }

    // Defends: the checked-in Starship asset is generated from the compact module table.
    #[test]
    fn checked_in_starship_config_matches_generator() {
        assert_eq!(
            include_str!("../yazelix_starship.toml"),
            render_yazelix_starship_config()
        );
    }

    // Defends: contextual shell decorations survive in Yazi without leaking labels or values.
    #[test]
    fn bundled_starship_context_modules_are_icon_only() {
        let raw = include_str!("../yazelix_starship.toml");

        for module in [
            "aws",
            "gcloud",
            "openstack",
            "azure",
            "kubernetes",
            "docker_context",
            "container",
            "terraform",
            "pulumi",
        ] {
            assert!(
                raw.contains(&format!("${module}\\")),
                "missing contextual sidebar module: {module}"
            );
        }

        let config = toml::from_str::<toml::Table>(raw).expect("sidebar Starship TOML");
        let aws = config["aws"].as_table().expect("AWS sidebar module");
        assert_eq!(aws.len(), 2);
        let format = aws["format"].as_str().expect("AWS format");
        assert_eq!(format, "[ $symbol]($style)");
        for value in ["$profile", "$region", "$duration"] {
            assert!(!format.contains(value), "AWS sidebar leaked {value}");
        }
    }

    // Defends: wide language and tool emoji stay out of the compact Yazi header.
    #[test]
    fn bundled_starship_prompt_avoids_wide_language_emoji() {
        let raw = include_str!("../yazelix_starship.toml");

        for emoji in [
            "📦", "🐍", "🦀", "☕", "🌙", "💎", "🐘", "🐦", "⚡", "⭐", "❄️",
        ] {
            assert!(
                !raw.contains(emoji),
                "emoji symbol leaked into sidebar Starship config: {emoji}"
            );
        }
        for symbol in [
            "", "", "", "", "", "", "", "", "", "", "", "", "", "",
        ] {
            assert!(
                raw.contains(symbol),
                "missing sidebar Starship icon: {symbol}"
            );
        }

        assert!(
            !raw.contains("$git_status \\"),
            "git_status should not inject an unconditional space before icon modules"
        );
        assert!(
            raw.contains("format = \"[ $symbol]($style)\""),
            "icon-only modules should carry their own leading separator"
        );
    }
}

use crate::app_core::Core;
use crate::files::gate;
use crate::files::models::FileItem;
use crate::files::workspace::read_directory_impl;
use crate::history::Source;
use crate::mcp::McpStatus;
use crate::settings::models::AppConfig;
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub struct NativeSettingsController {
    core: Arc<Core>,
}

impl NativeSettingsController {
    pub fn new(core: Arc<Core>) -> Self {
        Self { core }
    }

    pub fn config(&self) -> AppConfig {
        self.core.settings.get_config()
    }

    pub fn update(&self, new_config: AppConfig) -> Result<(), String> {
        self.core.settings.update_config(new_config)
    }

    pub fn set_theme(&self, theme: &str) -> Result<(), String> {
        let mut cfg = self.config();
        cfg.ui.theme = theme.to_owned();
        self.update(cfg)
    }

    pub fn set_editor_font(&self, family: &str, size_px: u32) -> Result<(), String> {
        let mut cfg = self.config();
        cfg.editor.font.font_family = family.to_owned();
        cfg.editor.font.font_size_base = size_px;
        self.update(cfg)
    }

    pub fn apply_mcp(&self) -> McpStatus {
        self.core.apply_mcp_settings()
    }
}

pub struct NativeTemplateService {
    core: Arc<Core>,
}

impl NativeTemplateService {
    pub fn new(core: Arc<Core>) -> Self {
        Self { core }
    }

    pub fn list_templates(&self, workspace_root: &Path) -> Vec<FileItem> {
        let folder = self.core.settings.get_config().templates.folder;
        let dir = workspace_root.join(folder);
        read_directory_impl(&dir).unwrap_or_default()
    }

    pub fn create_note_from_template(
        &self,
        workspace_root: &Path,
        template_rel_or_name: &str,
        target_rel_path: &str,
        date_iso: &str,
    ) -> Result<String, String> {
        let folder = self.core.settings.get_config().templates.folder;
        let template_path = if Path::new(template_rel_or_name).is_absolute() {
            Path::new(template_rel_or_name).to_path_buf()
        } else {
            let candidate = workspace_root.join(template_rel_or_name);
            if candidate.is_file() {
                candidate
            } else {
                let with_ext = if template_rel_or_name.ends_with(".md") {
                    template_rel_or_name.to_owned()
                } else {
                    format!("{template_rel_or_name}.md")
                };
                workspace_root.join(folder).join(with_ext)
            }
        };

        let raw_template = fs::read_to_string(&template_path)
            .map_err(|e| format!("failed to read template {}: {e}", template_path.display()))?;
        let note_title = Path::new(target_rel_path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Заметка".to_owned());

        let expanded = raw_template
            .replace("{{title}}", &note_title)
            .replace("{{date}}", date_iso);

        let target_full = workspace_root.join(target_rel_path);
        if let Some(parent) = target_full.parent() {
            let _ = fs::create_dir_all(parent);
        }
        gate::create(&self.core, &target_full, &expanded, Source::Me, None)
            .map_err(|e| format!("failed to create note from template: {e:?}"))?;
        Ok(expanded)
    }
}

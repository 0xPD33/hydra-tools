use anyhow::{Context, Result};
use std::path::Path;
use tera::Tera;

pub struct TemplateContext {
    pub port: u16,
    pub worktree: String,
    pub project_uuid: String,
    pub repo_root: String,
}

pub fn render(template_path: &Path, output_path: &Path, ctx: &TemplateContext) -> Result<()> {
    if !template_path.exists() {
        eprintln!(
            "Warning: Template {} not found, skipping env generation",
            template_path.display()
        );
        return Ok(());
    }

    let template_content = std::fs::read_to_string(template_path)
        .with_context(|| format!("Failed to read template {}", template_path.display()))?;

    let mut tera = Tera::default();
    tera.add_raw_template("env", &template_content)
        .context("Failed to parse template")?;

    let mut context = tera::Context::new();
    context.insert("port", &ctx.port);
    context.insert("worktree", &ctx.worktree);
    context.insert("project_uuid", &ctx.project_uuid);
    context.insert("repo_root", &ctx.repo_root);

    let rendered = tera
        .render("env", &context)
        .context("Failed to render template")?;

    std::fs::write(output_path, rendered)
        .with_context(|| format!("Failed to write {}", output_path.display()))?;

    Ok(())
}

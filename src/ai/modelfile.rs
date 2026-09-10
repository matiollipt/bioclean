use anyhow::Result;
use std::fs;
use std::path::PathBuf;

pub struct ModelfileParams {
    pub base_model: String,
    pub temperature: f32,
    pub top_k: u32,
    pub context_size: u32,
    pub system_prompt: String,
}

pub fn render_modelfile(params: &ModelfileParams) -> String {
    format!(
        "FROM {}\nPARAMETER temperature {}\nPARAMETER top_k {}\nPARAMETER num_ctx {}\nSYSTEM \"\"\"\n{}\n\"\"\"\n",
        params.base_model, params.temperature, params.top_k, params.context_size, params.system_prompt
    )
}

pub fn modelfile_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/clever".to_string());
    PathBuf::from(home).join(".config/bioclean/Modelfile")
}

pub fn write_modelfile(params: &ModelfileParams) -> Result<PathBuf> {
    let path = modelfile_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, render_modelfile(params))?;
    Ok(path)
}

pub fn suggested_model_name(base_model: &str) -> String {
    format!("bioclean-{}", base_model.split(':').next().unwrap_or(base_model))
}

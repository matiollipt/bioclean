use crate::ai::client::OllamaClient;
use crate::ai::fallback::safety_warning_fallback;
use crate::ai::prompts::{self, SAFETY_INTERLOCK_ROLE};
use crate::utils::system::ActionPreview;
use colored::*;

pub fn verify_safety_interlock(
    preview: &ActionPreview,
    target: &str,
    action: &str,
    ollama: &OllamaClient,
    model: &str,
    auto_yes: bool,
) -> bool {
    if auto_yes {
        println!(
            "{}",
            format!("[auto-confirmed] Proceeding without an interactive safety interlock for {}: {}", target, action).dimmed()
        );
        return true;
    }

    let warning_msg = if ollama.is_online() {
        let prompt = format!(
            "Target path: {}\nAction: {}\nGenerate a single concise warning sentence starting with 'I see you are about to...'",
            target, action
        );
        let system_prompt = prompts::compose(SAFETY_INTERLOCK_ROLE);
        match ollama.generate(model, &prompt, Some(&system_prompt), None) {
            Ok(ai_resp) if !ai_resp.is_empty() => ai_resp,
            _ => safety_warning_fallback(target, action),
        }
    } else {
        safety_warning_fallback(target, action)
    };

    println!("\n{}", "🤖 [AI Safety Interlock]".bright_yellow().bold());
    println!("{}", warning_msg.yellow());

    preview.confirm()
}

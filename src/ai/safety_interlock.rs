use crate::ai::client::OllamaClient;
use crate::ai::fallback::safety_warning_fallback;
use crate::ai::prompts::SAFETY_INTERLOCK_SYSTEM_PROMPT;
use crate::utils::system::confirm_prompt;
use colored::*;

pub fn verify_safety_interlock(
    target: &str,
    action: &str,
    ollama: &OllamaClient,
    model: &str,
    auto_yes: bool,
) -> bool {
    if auto_yes {
        return true;
    }

    let warning_msg = if ollama.is_online() {
        let prompt = format!(
            "Target path: {}\nAction: {}\nGenerate a single concise warning sentence starting with 'I see you are about to...'",
            target, action
        );
        match ollama.generate(model, &prompt, Some(SAFETY_INTERLOCK_SYSTEM_PROMPT), None) {
            Ok(ai_resp) if !ai_resp.is_empty() => ai_resp,
            _ => safety_warning_fallback(target, action),
        }
    } else {
        safety_warning_fallback(target, action)
    };

    println!("\n{}", "🤖 [AI Safety Interlock]".bright_yellow().bold());
    println!("{}", warning_msg.yellow());

    confirm_prompt("Proceed with operation?", false)
}

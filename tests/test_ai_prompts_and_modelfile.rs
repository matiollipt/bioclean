use bioclean::ai::modelfile::{render_modelfile, suggested_model_name, ModelfileParams};
use bioclean::ai::prompts::{compose, BASE_SYSTEM_PREAMBLE, DIAGNOSE_ROLE};
use bioclean::utils::disks::read_internal_disks;

#[test]
fn test_compose_prefixes_base_preamble() {
    let composed = compose(DIAGNOSE_ROLE);
    assert!(composed.starts_with(BASE_SYSTEM_PREAMBLE));
    assert!(composed.contains(DIAGNOSE_ROLE));
}

#[test]
fn test_render_modelfile_exact_format() {
    let params = ModelfileParams {
        base_model: "qwen2.5-coder:7b".to_string(),
        temperature: 0.2,
        top_k: 40,
        context_size: 4096,
        system_prompt: "You are a test assistant.".to_string(),
    };
    let rendered = render_modelfile(&params);
    assert_eq!(
        rendered,
        "FROM qwen2.5-coder:7b\nPARAMETER temperature 0.2\nPARAMETER top_k 40\nPARAMETER num_ctx 4096\nSYSTEM \"\"\"\nYou are a test assistant.\n\"\"\"\n"
    );
}

#[test]
fn test_suggested_model_name_strips_tag() {
    assert_eq!(suggested_model_name("qwen2.5-coder:7b"), "bioclean-qwen2.5-coder");
    assert_eq!(suggested_model_name("llama3"), "bioclean-llama3");
}

#[test]
fn test_read_internal_disks_does_not_panic() {
    // Real disk values are environment-dependent; this only asserts the call
    // completes without panicking and returns a well-formed (possibly empty) list.
    let disks = read_internal_disks();
    for d in disks {
        assert!(d.total_bytes >= d.available_bytes || d.total_bytes == 0);
    }
}

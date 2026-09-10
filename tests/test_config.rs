use bioclean::config::Config;

#[test]
fn test_config_defaults() {
    let cfg = Config::default();
    assert_eq!(cfg.ollama_url, "http://localhost:11434");
    assert_eq!(cfg.temperature, 0.2);
    assert_eq!(cfg.top_k, 40);
    assert_eq!(cfg.context_size, 4096);
    assert_eq!(cfg.report_formatter, "auto");
    assert!(cfg.safety_interlock);
    assert_eq!(cfg.log_vacuum_days, 7);
    assert_eq!(cfg.tmp_min_age_hours, 48);
}

#[test]
fn test_config_set_get_params() {
    let mut cfg = Config::default();

    // Ollama URL
    assert!(cfg.set_param("ollama_url", "http://127.0.0.1:11434").is_ok());
    assert_eq!(cfg.get_param("ollama_url").unwrap(), "http://127.0.0.1:11434");
    assert!(cfg.set_param("ollama_url", "invalid-url").is_err());

    // Temperature
    assert!(cfg.set_param("temperature", "0.75").is_ok());
    assert_eq!(cfg.get_param("temperature").unwrap(), "0.75");
    assert!(cfg.set_param("temperature", "2.5").is_err());
    assert!(cfg.set_param("temperature", "-0.1").is_err());

    // Top-K
    assert!(cfg.set_param("top_k", "50").is_ok());
    assert_eq!(cfg.get_param("top_k").unwrap(), "50");
    assert!(cfg.set_param("top_k", "0").is_err());

    // Context size
    assert!(cfg.set_param("context_size", "8192").is_ok());
    assert_eq!(cfg.get_param("context_size").unwrap(), "8192");
    assert!(cfg.set_param("context_size", "256").is_err());

    // Report formatter
    assert!(cfg.set_param("report_formatter", "glow").is_ok());
    assert_eq!(cfg.get_param("report_formatter").unwrap(), "glow");
    assert!(cfg.set_param("report_formatter", "batcat").is_ok());
    assert!(cfg.set_param("report_formatter", "invalid_fmt").is_err());

    // Safety interlock
    assert!(cfg.set_param("safety_interlock", "false").is_ok());
    assert_eq!(cfg.get_param("safety_interlock").unwrap(), "false");
    assert!(cfg.set_param("safety_interlock", "true").is_ok());
    assert_eq!(cfg.get_param("safety_interlock").unwrap(), "true");

    // Unknown key
    assert!(cfg.set_param("nonexistent_setting", "value").is_err());
}

#[test]
fn test_config_ollama_options_mapping() {
    let mut cfg = Config::default();
    cfg.temperature = 0.4;
    cfg.top_k = 60;
    cfg.context_size = 16384;

    let opts = cfg.ollama_options();
    assert_eq!(opts.temperature, Some(0.4));
    assert_eq!(opts.top_k, Some(60));
    assert_eq!(opts.num_ctx, Some(16384));
}

use ai_agent_common::config::SystemConfig;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_config_load_from_toml() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test_config.toml");

    let config_content = r#"
[indexing]
workspace_paths = ["/home/user/projects"]
personal_paths = ["/home/user/docs"]
system_paths = ["/usr/share/man"]
watch_enabled = true
chunk_size = 512

[rag]
max_results = 5
query_enhancement_model = "qwen2.5:7b"

[rag.reranking_weights]
conversation_boost = 1.5
recency_boost = 1.2
dependency_boost = 1.3

[orchestrator]
checkpoint_interval = "after_wave"

[[orchestrator.agents]]
name = "orchestrator"
model = "llama3.3:70b"
system_prompt = "You are a task orchestrator"
temperature = 0.7

[storage]
qdrant_url = "http://localhost:6333"
postgres_url = "postgresql://localhost/test_db"
redis_url = "redis://localhost:6379"
"#;

    fs::write(&config_path, config_content).unwrap();

    let config = SystemConfig::from_file(config_path.to_str().unwrap()).unwrap();

    assert_eq!(config.indexing.chunk_size, 512);
    assert_eq!(config.rag.max_results, 5);
    assert_eq!(config.storage.qdrant_url, "http://localhost:6333");
    assert_eq!(config.orchestrator.agents.len(), 1);
    assert_eq!(config.orchestrator.agents[0].name, "orchestrator");
}

#[test]
fn test_config_validation_invalid_chunk_size() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("invalid_config.toml");

    let config_content = r#"
[indexing]
workspace_paths = []
personal_paths = []
system_paths = []
watch_enabled = true
chunk_size = 0

[rag]
max_results = 5
query_enhancement_model = "qwen2.5:7b"

[rag.reranking_weights]
conversation_boost = 1.5
recency_boost = 1.2
dependency_boost = 1.3

[orchestrator]
checkpoint_interval = "after_wave"
agents = []

[storage]
qdrant_url = "http://localhost:6333"
postgres_url = "postgresql://localhost/test_db"
"#;

    fs::write(&config_path, config_content).unwrap();

    let result = SystemConfig::from_file(config_path.to_str().unwrap());
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("chunk_size"));
}

#[test]
fn test_config_validation_invalid_temperature() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("invalid_temp.toml");

    let config_content = r#"
[indexing]
workspace_paths = []
personal_paths = []
system_paths = []
watch_enabled = true
chunk_size = 512

[rag]
max_results = 5
query_enhancement_model = "qwen2.5:7b"

[rag.reranking_weights]
conversation_boost = 1.5
recency_boost = 1.2
dependency_boost = 1.3

[orchestrator]
checkpoint_interval = "after_wave"

[[orchestrator.agents]]
name = "test"
model = "llama3"
system_prompt = "test"
temperature = 3.0

[storage]
qdrant_url = "http://localhost:6333"
postgres_url = "postgresql://localhost/test_db"
"#;

    fs::write(&config_path, config_content).unwrap();

    let result = SystemConfig::from_file(config_path.to_str().unwrap());
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("temperature"));
}

#[test]
fn test_get_agent_config() {
    let mut config = SystemConfig::default();
    config.orchestrator.agents.push(ai_agent_common::config::AgentConfig {
        name: "test_agent".to_string(),
        model: "test_model".to_string(),
        system_prompt: "test prompt".to_string(),
        temperature: 0.5,
    });

    assert!(config.get_agent_config("test_agent").is_some());
    assert!(config.get_agent_config("nonexistent").is_none());
}


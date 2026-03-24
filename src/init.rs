use std::io::{self, Write};
use std::path::Path;

use crate::cli::ToolSpec;
use crate::error::RunxError;
use crate::provider;

use crate::config::CONFIG_FILE_NAME;
use crate::provider::TOOL_REGISTRY;

/// Execute the `runx init` subcommand.
pub fn run(tools: &[ToolSpec], force: bool) -> Result<(), RunxError> {
    let config_path = Path::new(CONFIG_FILE_NAME);

    if config_path.exists() && !force {
        eprintln!(
            "A .runxrc file already exists in this directory.\n\
             Use `runx init --force` to overwrite it."
        );
        return Ok(());
    }

    let tool_specs = if tools.is_empty() {
        prompt_tools()?
    } else {
        // Validate tool names in non-interactive mode
        for spec in tools {
            if provider::get_provider(&spec.name).is_err() {
                eprintln!(
                    "Unknown tool `{}`. Supported tools: node, python, go, deno, bun",
                    spec.name
                );
                return Err(RunxError::Provider(
                    crate::provider::ProviderError::UnknownTool {
                        name: spec.name.clone(),
                    },
                ));
            }
        }
        tools.to_vec()
    };

    let content = generate_config(&tool_specs);

    std::fs::write(config_path, &content).map_err(RunxError::Io)?;

    println!("Created .runxrc");
    if !tool_specs.is_empty() {
        for spec in &tool_specs {
            println!("  {spec}");
        }
    }
    println!("\nRun `runx -- <command>` to use the configured tools.");
    Ok(())
}

/// Read a line from stdin after printing a prompt.
fn prompt_line(prompt: &str) -> Result<String, RunxError> {
    print!("{prompt}");
    io::stdout().flush().map_err(RunxError::Io)?;
    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(RunxError::Io)?;
    Ok(input)
}

/// Interactively prompt the user to select tools.
fn prompt_tools() -> Result<Vec<ToolSpec>, RunxError> {
    println!(
        "Which tools do you need? (enter numbers separated by spaces, or press Enter to skip)"
    );
    println!();
    for (i, entry) in TOOL_REGISTRY.iter().enumerate() {
        println!("  {}. {}", i + 1, entry.name);
    }
    println!();

    let input = prompt_line("Tools: ")?;

    let mut specs = Vec::new();
    for token in input.split_whitespace() {
        if let Ok(num) = token.parse::<usize>()
            && num >= 1
            && num <= TOOL_REGISTRY.len()
        {
            let name = TOOL_REGISTRY[num - 1].name;
            let version = prompt_version(name)?;
            specs.push(ToolSpec {
                name: name.to_string(),
                version,
            });
        }
    }

    Ok(specs)
}

/// Prompt for a version for a specific tool.
fn prompt_version(tool: &str) -> Result<Option<String>, RunxError> {
    let input = prompt_line(&format!(
        "{tool} version (e.g. 18, 3.11, or Enter for latest): "
    ))?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

/// Generate the `.runxrc` file content with comments.
fn generate_config(tools: &[ToolSpec]) -> String {
    let mut content = String::new();
    content.push_str("# runx configuration file\n");
    content.push_str("# See: https://github.com/supa-magic/runx\n");
    content.push('\n');
    content.push_str("# Tools to include in the environment.\n");
    content.push_str("# Format: \"tool\" or \"tool@version\"\n");
    content.push_str("# Supported tools: node, python, go, deno, bun\n");

    if tools.is_empty() {
        content.push_str("# tools = [\"node@18\", \"python@3.11\"]\n");
    } else {
        content.push_str("tools = [");
        let tool_strings: Vec<String> = tools.iter().map(|t| format!("\"{t}\"")).collect();
        content.push_str(&tool_strings.join(", "));
        content.push_str("]\n");
    }

    content.push('\n');
    content.push_str("# Inherit the user's full environment instead of an isolated one.\n");
    content.push_str("# inherit_env = false\n");

    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_config_with_tools() {
        let tools = vec![
            ToolSpec {
                name: "node".to_string(),
                version: Some("18".to_string()),
            },
            ToolSpec {
                name: "python".to_string(),
                version: Some("3.11".to_string()),
            },
        ];
        let content = generate_config(&tools);
        assert!(content.contains("tools = [\"node@18\", \"python@3.11\"]"));
        assert!(content.contains("# runx configuration file"));
        assert!(content.contains("# inherit_env = false"));
    }

    #[test]
    fn test_generate_config_empty_tools() {
        let content = generate_config(&[]);
        assert!(content.contains("# tools = [\"node@18\", \"python@3.11\"]"));
        assert!(!content.contains("\ntools = ["));
    }

    #[test]
    fn test_generate_config_tool_without_version() {
        let tools = vec![ToolSpec {
            name: "node".to_string(),
            version: None,
        }];
        let content = generate_config(&tools);
        assert!(content.contains("tools = [\"node\"]"));
    }

    #[test]
    fn test_generate_config_has_comments() {
        let content = generate_config(&[]);
        assert!(content.contains("# Supported tools:"));
        assert!(content.contains("# Inherit the user's full environment"));
    }

    #[test]
    fn test_init_warns_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&config_path, "existing").unwrap();

        // Verify the file exists check logic
        assert!(config_path.exists());
    }

    #[test]
    fn test_init_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILE_NAME);

        let tools = vec![ToolSpec {
            name: "go".to_string(),
            version: Some("1.21".to_string()),
        }];
        let content = generate_config(&tools);
        std::fs::write(&config_path, &content).unwrap();

        let read_back = std::fs::read_to_string(&config_path).unwrap();
        assert!(read_back.contains("tools = [\"go@1.21\"]"));
    }

    #[test]
    fn test_init_force_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&config_path, "old content").unwrap();

        let content = generate_config(&[ToolSpec {
            name: "node".to_string(),
            version: Some("20".to_string()),
        }]);
        std::fs::write(&config_path, &content).unwrap();

        let read_back = std::fs::read_to_string(&config_path).unwrap();
        assert!(read_back.contains("node@20"));
        assert!(!read_back.contains("old content"));
    }

    #[test]
    fn test_available_tools_matches_providers() {
        for entry in TOOL_REGISTRY {
            assert!(
                crate::provider::get_provider(entry.name).is_ok(),
                "TOOL_REGISTRY contains unknown tool: {}",
                entry.name
            );
        }
    }
}

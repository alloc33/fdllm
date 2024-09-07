use copypasta::{ClipboardContext, ClipboardProvider};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Deserialize)]
struct Config {
    files: Vec<String>,
    project: Option<Project>,
}

#[derive(Deserialize)]
struct Project {
    path: String,
    tree_level: Option<u32>,
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Ok(home_dir) = env::var("HOME") {
        return PathBuf::from(path.replacen("~", &home_dir, 1));
    }
    PathBuf::from(path)
}

fn get_config_path() -> PathBuf {
    let home_dir = env::var("HOME").expect("Failed to get $HOME directory");
    let config_dir = Path::new(&home_dir).join("fdllm");
    let config_file = config_dir.join("config.toml");

    if !config_file.exists() {
        fs::create_dir_all(&config_dir).expect("Failed to create fdllm directory");
        let default_config = r#"files = ["~/Desktop/my_test_file2.txt", "src/main.rs"]

[project]
path = ""
tree_level = 3
"#;
        fs::write(&config_file, default_config).expect("Failed to write default config.toml");
        println!("Default config.toml created at {}", config_file.display());
    }

    config_file
}

fn load_config(config_path: &Path) -> Config {
    let config_content = fs::read_to_string(config_path)
        .unwrap_or_else(|_| panic!("Failed to read config file: {}", config_path.display()));
    toml::from_str(&config_content)
        .unwrap_or_else(|_| panic!("Failed to parse config file: {}", config_path.display()))
}

fn run_tree_command(project_path: &str, tree_level: Option<u32>) -> Option<String> {
    let mut command = Command::new("eza");
    command
        .arg("--tree")
        .arg("--icons")
        .arg("--git")
        .arg(project_path);

    if let Some(level) = tree_level {
        command.arg("-L").arg(level.to_string());
    }

    let output = command.output().ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        eprintln!("Failed to run eza command");
        None
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let config_path = get_config_path();

    let config = load_config(&config_path);

    let files_to_copy: Vec<String> = if args.len() > 1 {
        args[1..].to_vec()
    } else {
        if config.files.is_empty() {
            eprintln!("No files provided via arguments or config.");
            std::process::exit(1);
        }
        config.files
    };

    let mut combined_content = String::new();

    if let Some(project) = &config.project {
        let project_path = expand_tilde(&project.path);
        if project_path.exists() {
            if let Some(tree_output) =
                run_tree_command(&project_path.to_string_lossy(), project.tree_level)
            {
                combined_content.push_str(&format!(
                    "Project Tree: {}\n{}\n",
                    project_path.display(),
                    tree_output
                )); 
            }
        } else {
            eprintln!("Project path not found: {}", project_path.display());
        }
    }

    for file in files_to_copy {
        let file_path = expand_tilde(&file); // Expand the tilde (~)
        if file_path.exists() {
            let file_content = fs::read_to_string(&file_path)
                .unwrap_or_else(|_| panic!("Failed to read file: {}", file));
            combined_content.push_str(&format!("{}:\n{}\n", file, file_content));
        } else {
            eprintln!("File not found: {}", file_path.display());
        }
    }

    if combined_content.is_empty() {
        eprintln!("No valid files or project tree found to copy.");
        std::process::exit(1);
    }

    let mut ctx = ClipboardContext::new().unwrap();
    ctx.set_contents(combined_content).unwrap();

    println!("File contents and project tree copied to clipboard.");
}

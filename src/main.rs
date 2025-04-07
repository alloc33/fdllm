use copypasta::{ClipboardContext, ClipboardProvider};
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Deserialize, Default)]
struct Config {
    // Legacy fields for backward compatibility
    files: Option<Vec<String>>,
    directories: Option<Vec<String>>,
    project: Option<Project>,
    // New profiles field
    profiles: Option<HashMap<String, Profile>>,
}

#[derive(Deserialize)]
struct Profile {
    files: Option<Vec<String>>,
    directories: Option<Vec<String>>,
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
        let default_config = r#"# Default configuration (used when no profile is specified)
files = ["~/Desktop/my_test_file.txt"]
directories = ["~/example_dir"]

[project]
path = ""
tree_level = 3

# Example profile configurations
[profiles.project1]
files = ["~/project1/main.rs"]
directories = ["~/project1/src"]

[profiles.project1.project]
path = "~/project1"
tree_level = 2

[profiles.project2]
files = ["~/project2/app.js"]
directories = ["~/project2/lib"]

[profiles.project2.project]
path = "~/project2"
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
    
    match toml::from_str(&config_content) {
        Ok(config) => config,
        Err(err) => {
            panic!("Failed to parse config file: {}\nError: {}", config_path.display(), err);
        }
    }
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

fn collect_files_from_directory(dir_path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    
    // File extensions or names to exclude
    let excluded_files = [".DS_Store", ".git", ".gitignore", "target"];
    
    // Add your needed extensions
    let valid_extensions = [
        ".rs", ".toml", ".json", ".yaml", ".yml", ".md", ".txt", 
        ".c", ".h", ".cpp", ".hpp", ".js", ".ts", ".py", ".go", ".sh",
        ".csv", ".log" // Add your specific file extensions
    ];
    
    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();
            
            // Skip excluded files/directories
            if excluded_files.iter().any(|&excluded| file_name.contains(excluded)) {
                continue;
            }
            
            if path.is_file() {
                // Check if the file has a valid extension
                if let Some(extension) = path.extension() {
                    let ext = format!(".{}", extension.to_string_lossy());
                    if valid_extensions.contains(&ext.as_str()) {
                        files.push(path);
                    } else {
                        // Debug print to help understand what's being filtered
                        println!("Skipping file with unsupported extension: {}", path.display());
                    }
                }
            } else if path.is_dir() {
                // Recursively collect files from subdirectories
                let mut subdir_files = collect_files_from_directory(&path);
                files.append(&mut subdir_files);
            }
        }
    }
    
    // Debug print to help understand what files were found
    println!("Found {} files in directory: {}", files.len(), dir_path.display());
    
    files
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let config_path = get_config_path();
    let config = load_config(&config_path);
    
    // Determine which profile to use
    let profile_name = if args.len() > 1 {
        Some(args[1].clone())
    } else {
        None
    };
    
    // Files and directories to process
    let mut files_to_copy = Vec::new();
    let mut directories_to_process = Vec::new();
    let mut project_config: Option<&Project> = None;
    
    // Use the specified profile if it exists
    if let Some(profile_name) = profile_name {
        if let Some(profiles) = &config.profiles {
            if let Some(profile) = profiles.get(&profile_name) {
                // Use profile's files
                if let Some(profile_files) = &profile.files {
                    files_to_copy.extend(profile_files.clone());
                }
                
                // Use profile's directories
                if let Some(profile_dirs) = &profile.directories {
                    directories_to_process.extend(profile_dirs.clone());
                }
                
                // Use profile's project
                project_config = profile.project.as_ref();
                
                println!("Using profile: {}", profile_name);
            } else {
                eprintln!("Profile '{}' not found in config", profile_name);
                std::process::exit(1);
            }
        } else {
            eprintln!("No profiles defined in config");
            std::process::exit(1);
        }
    } else {
        // Use default config (for backward compatibility)
        if let Some(config_files) = &config.files {
            files_to_copy.extend(config_files.clone());
        }
        
        if let Some(config_dirs) = &config.directories {
            directories_to_process.extend(config_dirs.clone());
        }
        
        project_config = config.project.as_ref();
        
        println!("Using default configuration");
    }
    
    // Collect files from directories
    for dir in &directories_to_process {
        let dir_path = expand_tilde(dir);
        if dir_path.exists() && dir_path.is_dir() {
            let files_in_dir = collect_files_from_directory(&dir_path);
            for file in files_in_dir {
                files_to_copy.push(file.to_string_lossy().to_string());
            }
        } else {
            eprintln!("Directory not found or not a directory: {}", dir_path.display());
        }
    }
    
    if files_to_copy.is_empty() {
        eprintln!("No files provided via config or directories");
        std::process::exit(1);
    }
    
    let mut combined_content = String::new();
    
    // Add project tree if specified
    if let Some(project) = project_config {
        let project_path = expand_tilde(&project.path);
        if project_path.exists() {
            if let Some(tree_output) = run_tree_command(&project_path.to_string_lossy(), project.tree_level) {
                combined_content.push_str(&format!(
                    "# NOTE: Project Tree: {}\n{}\n",
                    project_path.display(),
                    tree_output
                ));
            }
        } else {
            eprintln!("Project path not found: {}", project_path.display());
        }
    }
    
    // Process files
    for file in files_to_copy {
        let file_path = expand_tilde(&file);
        if file_path.exists() && file_path.is_file() {
            match fs::read_to_string(&file_path) {
                Ok(file_content) => {
                    combined_content.push_str(&format!("# NOTE: {}:\n{}\n", file, file_content));
                },
                Err(err) => {
                    eprintln!("Failed to read file {}: {}", file_path.display(), err);
                }
            }
        } else {
            eprintln!("File not found or not a file: {}", file_path.display());
        }
    }
    
    if combined_content.is_empty() {
        eprintln!("No valid files or project tree found to copy");
        std::process::exit(1);
    }
    
    // Copy to clipboard
    match ClipboardContext::new() {
        Ok(mut ctx) => {
            if let Err(err) = ctx.set_contents(combined_content) {
                eprintln!("Failed to copy to clipboard: {}", err);
                std::process::exit(1);
            }
            println!("File contents and project tree copied to clipboard");
        },
        Err(err) => {
            eprintln!("Failed to access clipboard: {}", err);
            std::process::exit(1);
        }
    }
}

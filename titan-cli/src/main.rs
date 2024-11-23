use clap::{Parser, Subcommand};
use colored::Colorize;
use include_dir::{include_dir, Dir, DirEntry};
use std::{
    env, // Import to get the current directory
    fs::{self, File},
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

// Templates and files to embed
const APP_BIN_TEMPLATE: &str = include_str!("../template/app/app_bin_template.txt");
const APP_LIB_TEMPLATE: &str = include_str!("../template/app/app_lib_template.rs");
const APP_CARGO_TEMPLATE: &str = include_str!("../template/app/app_cargo_template.toml");
const APP_BUILD_TEMPLATE: &str = include_str!("../template/app/app_build_template.rs");
const LSP_TEMPLATE: &str = include_str!("../template/lsp/lsp_template.rs");
const HELIX_IGNORE_TEMPLATE: &str = include_str!("../template/helix/helix_.ignore_template");
const HELIX_LANGUAGES_TEMPLATE: &str =
    include_str!("../template/helix/helix_languages_template.toml");
const HELIX_ADASTRA_GRAMMARS_DIR: Dir<'_> =
    include_dir!("$CARGO_MANIFEST_DIR/template/scripting/adastra");
const HELIX_ADASTRA_QUERIES_TEMPLATE: &str = include_str!("../template/scripting/highlights.scm");

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init {
        #[arg(short, long)]
        name: String,
        /// Enable Helix integration
        #[arg(short, long)]
        helix: bool,
    },
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init { name, helix } => {
            let init_result = init(name, *helix);

            if init_result.is_err() {
                deinit(name)?;
            }

            init_result
        }
    }
}

fn deinit(app_name: &str) -> std::io::Result<()> {
    let remove_app_dir = env::current_dir()?.join(app_name);

    if remove_app_dir.exists() {
        fs::remove_dir_all(remove_app_dir)?;
        println!("Removed created project: {}", app_name);
    }

    Ok(())
}

fn init(name: &String, helix: bool) -> std::io::Result<()> {
    // Get the directory from which the command was called
    let current_dir = env::current_dir()?;
    println!("Current directory: {}", current_dir.display());

    // Create the path to the new project based on the calling directory
    let name = name.replace("-", "_");
    let app_dir = current_dir.join(&name);

    println!("Creating project in: {}", app_dir.display());

    // Initialize a new cargo project in the current directory
    Command::new("cargo")
        .arg("init")
        .arg("--bin")
        .arg(&name)
        .current_dir(&current_dir) // Ensure we're in the directory the command was called from
        .stdout(Stdio::piped())
        .spawn()?
        .wait()?;

    println!("{}", "Project created".green());

    // Add dependencies
    Command::new("cargo")
        .arg("add")
        .arg("--path")
        .arg("/home/stormblessed/dev/titan/titan")
        .current_dir(&app_dir) // Now run in the new project's directory
        .stdout(Stdio::piped())
        .spawn()?
        .wait()?;

    println!("{}", "Dependency added: titan".green());

    Command::new("cargo")
        .arg("add")
        .arg("ad-astra")
        .current_dir(&app_dir) // Run in the new project's directory
        .stdout(Stdio::piped())
        .spawn()?
        .wait()?;

    println!("{}", "Dependency added: ad-astra".green());

    create_project_dirs(&app_dir, &name)?;

    replace_main_rs(&app_dir, &name)?;

    add_lib_rs(&app_dir, &name)?;

    replace_cargo_toml(&app_dir, &name)?;

    replace_build_rs(&app_dir, &name)?;

    setup_lsp_tool(&app_dir, &name)?;

    if helix {
        println!("Setting up Helix integration...");
        setup_helix(&app_dir)?;
    }

    Ok(())
}

fn create_project_dirs(app_dir: &Path, _app_name: &str) -> std::io::Result<()> {
    let content_dir = app_dir.join("content");

    if let Err(e) = fs::create_dir_all(&content_dir) {
        eprintln!(
            "Failed to create content directory: {}",
            e.to_string().red()
        );
        return Err(e);
    }

    println!("Content directory created at: {}", content_dir.display());

    let default_script = content_dir.join("welcome.aa");

    if let Err(e) = File::create(&default_script) {
        eprintln!("Failed to create script file: {}", e.to_string().red());
        return Err(e);
    }

    println!("Script file created at: {}", default_script.display());

    Ok(())
}

fn replace_main_rs(app_dir: &Path, app_name: &str) -> std::io::Result<()> {
    let main_rs_path = app_dir.join("src/main.rs");

    let lib_name = app_name.replace("-", "_");

    let main_rs_content = APP_BIN_TEMPLATE.replace("{lib_name}", &lib_name);

    let mut main_rs_file = File::create(&main_rs_path)?;
    main_rs_file.write_all(main_rs_content.as_bytes())?;

    println!("Main app's main.rs created for: {}", app_name);
    Ok(())
}
fn add_lib_rs(app_dir: &Path, app_name: &str) -> std::io::Result<()> {
    let librs_path = app_dir.join("src/lib.rs");

    let librs_content = APP_LIB_TEMPLATE.replace("{app_name}", &to_camel_case(app_name));

    let mut librs_file = File::create(&librs_path)?;
    librs_file.write_all(librs_content.as_bytes())?;

    println!("Main app's main.rs created for: {}", app_name);
    Ok(())
}

fn replace_cargo_toml(app_dir: &Path, app_name: &str) -> std::io::Result<()> {
    let cargo_toml_path = app_dir.join("Cargo.toml");

    let mut cargo_toml = fs::read_to_string(&cargo_toml_path).expect("Failed to read Cargo.toml");

    cargo_toml.push('\n');
    cargo_toml.push_str(APP_CARGO_TEMPLATE);

    let bin_name = format!("{}_bin", app_name.replace("-", "_"));
    let lib_name = app_name.replace("-", "_");

    let cargo_toml = cargo_toml.replace("{bin_name}", &bin_name);
    let cargo_toml = cargo_toml.replace("{lib_name}", &lib_name);

    fs::write(cargo_toml_path, cargo_toml).expect("Failed to write updated Cargo.toml");

    println!("Cargo.toml updated with dependencies for: {}", app_name);
    Ok(())
}

fn replace_build_rs(app_dir: &Path, app_name: &str) -> std::io::Result<()> {
    let build_rs_path = app_dir.join("build.rs");

    let build_rs_content = APP_BUILD_TEMPLATE.replace("{app_name}", &to_camel_case(app_name));

    let mut build_rs_file = File::create(&build_rs_path)?;
    build_rs_file.write_all(build_rs_content.as_bytes())?;

    println!("Main app's main.rs created for: {}", app_name);
    Ok(())
}

fn setup_lsp_tool(app_dir: &Path, app_name: &str) -> std::io::Result<()> {
    let lsp_dir = app_dir.join("tools/titan-lsp");

    Command::new("cargo")
        .arg("init")
        .arg("--bin")
        .arg(&lsp_dir)
        .stdout(Stdio::piped())
        .spawn()?
        .wait()?;

    println!("LSP tool binary project created in: {}", lsp_dir.display());

    Command::new("cargo")
        .arg("add")
        .arg("ad-astra")
        .current_dir(&lsp_dir)
        .stdout(Stdio::piped())
        .spawn()?
        .wait()?;

    println!("Added `ad-astra` to the LSP project.");

    Command::new("cargo")
        .arg("add")
        .arg(app_name)
        .arg("--path")
        .arg(app_dir)
        .current_dir(&lsp_dir)
        .stdout(Stdio::piped())
        .spawn()?
        .wait()?;

    println!("Linked the user app to the LSP project.");

    let lsp_main_rs_path = lsp_dir.join("src/main.rs");

    let lsp_main_rs_content = LSP_TEMPLATE.replace("{app_name}", &to_camel_case(app_name));

    let mut lsp_main_rs_file = File::create(&lsp_main_rs_path)?;
    lsp_main_rs_file.write_all(lsp_main_rs_content.as_bytes())?;

    println!("LSP tool's main.rs created for: {}", app_name);
    Ok(())
}

fn setup_helix(app_dir: &Path) -> std::io::Result<()> {
    let helix_config_dir = app_dir.join(".helix");
    let helix_grammar_dir = helix_config_dir.join("runtime/grammars/sources/adastra");
    let helix_queries_dir = helix_config_dir.join("runtime/queries/adastra");

    let app_absolute_path = app_dir
        .canonicalize()?
        .to_str()
        .unwrap()
        .to_string();

    println!("Helix config dir: {}", helix_config_dir.display());
    println!("Helix grammar dir: {}", helix_grammar_dir.display());
    println!("Helix queries dir: {}", helix_queries_dir.display());

    println!("Creating Helix directories...");

    std::fs::create_dir_all(&helix_grammar_dir)?;
    std::fs::create_dir_all(&helix_queries_dir)?;

    if !helix_grammar_dir.exists() {
        panic!("Helix grammar directory not created!");
    }

    if !helix_queries_dir.exists() {
        panic!("Helix queries directory not created!");
    }

    let helix_ignore_file = app_dir.join(".ignore");
    let mut ignore_file = File::create(&helix_ignore_file)?;
    ignore_file.write_all(HELIX_IGNORE_TEMPLATE.as_bytes())?;

    let helix_languages_path = helix_config_dir.join("languages.toml");
    let helix_languages_content =
        HELIX_LANGUAGES_TEMPLATE.replace("{app_path}", &app_absolute_path);
    let mut languages_file = File::create(&helix_languages_path)?;
    languages_file.write_all(helix_languages_content.as_bytes())?;

    println!(
        "languages.toml created at: {}",
        helix_languages_path.display()
    );

    let helix_queries_path = helix_queries_dir.join("highlights.scm");
    let mut queries_file = File::create(&helix_queries_path)?;
    queries_file.write_all(HELIX_ADASTRA_QUERIES_TEMPLATE.as_bytes())?;

    println!(
        "highlights.scm created at: {}",
        helix_queries_path.display()
    );

    unpack_files(
        helix_grammar_dir.to_str().unwrap(),
        &HELIX_ADASTRA_GRAMMARS_DIR,
    )?;

    println!("{}", "Grammars unpacked.".green());

    Command::new("hx")
        .arg("--grammar")
        .arg("build")
        .current_dir(app_dir)
        .stdout(Stdio::piped())
        .spawn()?
        .wait()?;

    println!("{}", "Helix grammars built successfully".green());
    Ok(())
}

fn unpack_files(target_directory: &str, unpack_directory: &Dir<'_>) -> std::io::Result<()> {
    for entry in unpack_directory.entries() {
        let entry_path = entry.path();
        let target_path = Path::new(target_directory).join(entry_path.file_name().unwrap());

        match entry {
            DirEntry::Dir(dir) => {
                fs::create_dir_all(&target_path)?;

                unpack_files(target_path.to_str().unwrap(), dir)?;
            }
            DirEntry::File(file) => {
                if let Some(parent) = target_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                let mut output_file = File::create(&target_path)?;
                output_file.write_all(file.contents())?;
            }
        }
    }
    Ok(())
}
fn to_camel_case(input: &str) -> String {
    input
        .split(|c: char| c == '_' || c == '-' || c.is_whitespace()) // Split by underscores, hyphens, or spaces
        .map(|word| {
            let mut c = word.chars();
            match c.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<String>()
}

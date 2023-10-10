use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rocket::data::{Data, ToByteUnit};
use rocket::fs::NamedFile;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::tokio::fs;

use crate::utils::lib::{get_file_ext, get_file_path, CAIRO_DIR, CASM_ROOT, SIERRA_ROOT};

#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct CompileResponse {
    pub status: String,
    pub message: String,
    pub file_content: String,
}

#[derive(Serialize, Deserialize)]
pub struct FileContentMap {
    pub file_name: String,
    pub file_content: String,
}

#[derive(Serialize, Deserialize)]
pub struct ScarbCompileResponse {
    pub status: String,
    pub message: String,
    pub file_content_map_array: Vec<FileContentMap>,
}

#[derive(Debug)]
pub enum ApiCommand {
    CairoVersion,
    SierraCompile(PathBuf),
    CasmCompile(PathBuf),
    ScarbCompile(PathBuf),
    #[allow(dead_code)]
    Shutdown,
}

pub enum ApiCommandResult {
    CairoVersion(String),
    CasmCompile(CompileResponse),
    SierraCompile(CompileResponse),
    ScarbCompile(ScarbCompileResponse),
    #[allow(dead_code)]
    Shutdown,
}

pub async fn dispatch_command(command: ApiCommand) -> Result<ApiCommandResult, String> {
    match command {
        ApiCommand::CairoVersion => match do_cairo_version() {
            Ok(result) => Ok(ApiCommandResult::CairoVersion(result)),
            Err(e) => Err(e),
        },
        ApiCommand::ScarbCompile(remix_file_path) => {
            match do_scarb_compile(remix_file_path).await {
                Ok(result) => Ok(ApiCommandResult::ScarbCompile(result.into_inner())),
                Err(e) => Err(e),
            }
        }
        ApiCommand::SierraCompile(remix_file_path) => {
            match do_compile_to_sierra(remix_file_path).await {
                Ok(compile_response) => Ok(ApiCommandResult::SierraCompile(
                    compile_response.into_inner(),
                )),
                Err(e) => Err(e),
            }
        }
        ApiCommand::CasmCompile(remix_file_path) => {
            match do_compile_to_casm(remix_file_path).await {
                Json(compile_response) => Ok(ApiCommandResult::CasmCompile(compile_response)),
            }
        }
        ApiCommand::Shutdown => Ok(ApiCommandResult::Shutdown),
    }
}

/// Upload a data file
///
pub async fn do_save_code(file: Data<'_>, remix_file_path: PathBuf) -> String {
    let remix_file_path = match remix_file_path.to_str() {
        Some(path) => path.to_string(),
        None => {
            return "".to_string();
        }
    };

    let file_path = get_file_path(&remix_file_path);

    // create file directory from file path
    match file_path.parent() {
        Some(parent) => match fs::create_dir_all(parent).await {
            Ok(_) => {
                println!("LOG: Created directory: {:?}", parent);
            }
            Err(e) => {
                println!("LOG: Error creating directory: {:?}", e);
            }
        },
        None => {
            println!("LOG: Error creating directory");
        }
    }

    // Modify to zip and unpack.
    let saved_file = file.open(128_i32.gibibytes()).into_file(&file_path).await;

    match saved_file {
        Ok(_) => {
            println!("LOG: File saved successfully");
            match file_path.to_str() {
                Some(path) => path.to_string(),
                None => "".to_string(),
            }
        }
        Err(e) => {
            println!("LOG: Error saving file: {:?}", e);
            "".to_string()
            // set the response with not ok code.
        }
    }
}

/// Compile a given file to Sierra bytecode
///
pub async fn do_compile_to_sierra(
    remix_file_path: PathBuf,
) -> Result<Json<CompileResponse>, String> {
    let remix_file_path = match remix_file_path.to_str() {
        Some(path) => path.to_string(),
        None => {
            return Ok(Json(CompileResponse {
                file_content: "".to_string(),
                message: "File path not found".to_string(),
                status: "FileNotFound".to_string(),
            }));
        }
    };

    // check if the file has .cairo extension
    match get_file_ext(&remix_file_path) {
        ext if ext == "cairo" => {
            println!("LOG: File extension is cairo");
        }
        _ => {
            println!("LOG: File extension not supported");
            return Ok(Json(CompileResponse {
                file_content: "".to_string(),
                message: "File extension not supported".to_string(),
                status: "FileExtensionNotSupported".to_string(),
            }));
        }
    }

    let file_path = get_file_path(&remix_file_path);

    let sierra_remix_path = remix_file_path.replace(&get_file_ext(&remix_file_path), "sierra");

    let mut compile = Command::new("cargo");
    compile.current_dir(CAIRO_DIR);

    // replace .cairo with
    let sierra_path = Path::new(SIERRA_ROOT).join(&sierra_remix_path);

    // create directory for sierra file
    match sierra_path.parent() {
        Some(parent) => match fs::create_dir_all(parent).await {
            Ok(_) => {
                println!("LOG: Created directory: {:?}", parent);
            }
            Err(e) => {
                println!("LOG: Error creating directory: {:?}", e);
            }
        },
        None => {
            println!("LOG: Error creating directory");
        }
    }

    let result = compile
        .arg("run")
        .arg("--release")
        .arg("--bin")
        .arg("starknet-compile")
        .arg("--")
        .arg(&file_path)
        .arg(&sierra_path)
        .arg("--single-file")
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to execute starknet-compile");

    println!("LOG: ran command:{:?}", compile);

    let output = result.wait_with_output().expect("Failed to wait on child");

    Ok(Json(CompileResponse {
        file_content: match NamedFile::open(&sierra_path).await.ok() {
            Some(file) => match file.path().to_str() {
                Some(path) => match fs::read_to_string(path.to_string()).await {
                    Ok(sierra) => sierra.to_string(),
                    Err(e) => e.to_string(),
                },
                None => "".to_string(),
            },
            None => "".to_string(),
        },
        message: String::from_utf8(output.stderr)
            .unwrap()
            .replace(&file_path.to_str().unwrap().to_string(), &remix_file_path)
            .replace(
                &sierra_path.to_str().unwrap().to_string(),
                &sierra_remix_path,
            ),
        status: match output.status.code() {
            Some(0) => "Success".to_string(),
            Some(_) => "CompilationFailed".to_string(),
            None => "UnknownError".to_string(),
        },
    }))
}

/// Compile source file to CASM
///
pub async fn do_compile_to_casm(remix_file_path: PathBuf) -> Json<CompileResponse> {
    let remix_file_path = match remix_file_path.to_str() {
        Some(path) => path.to_string(),
        None => {
            return Json(CompileResponse {
                file_content: "".to_string(),
                message: "File path not found".to_string(),
                status: "FileNotFound".to_string(),
            });
        }
    };

    // check if the file has .sierra extension
    match get_file_ext(&remix_file_path) {
        ext if ext == "sierra" => {
            println!("LOG: File extension is sierra");
        }
        _ => {
            println!("LOG: File extension not supported");
            return Json(CompileResponse {
                file_content: "".to_string(),
                message: "File extension not supported".to_string(),
                status: "FileExtensionNotSupported".to_string(),
            });
        }
    }

    let file_path = get_file_path(&remix_file_path);

    let casm_remix_path = remix_file_path.replace(&get_file_ext(&remix_file_path), "casm");

    let mut compile = Command::new("cargo");
    compile.current_dir(CAIRO_DIR);

    let casm_path = Path::new(CASM_ROOT).join(&casm_remix_path);

    // create directory for casm file
    match casm_path.parent() {
        Some(parent) => match fs::create_dir_all(parent).await {
            Ok(_) => {
                println!("LOG: Created directory: {:?}", parent);
            }
            Err(e) => {
                println!("LOG: Error creating directory: {:?}", e);
            }
        },
        None => {
            println!("LOG: Error creating directory");
        }
    }

    let result = compile
        .arg("run")
        .arg("--release")
        .arg("--bin")
        .arg("starknet-sierra-compile")
        .arg("--")
        .arg(&file_path)
        .arg(&casm_path)
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to execute starknet-sierra-compile");

    println!("LOG: ran command:{:?}", compile);

    let output = result.wait_with_output().expect("Failed to wait on child");

    Json(CompileResponse {
        file_content: match NamedFile::open(&casm_path).await.ok() {
            Some(file) => match file.path().to_str() {
                Some(path) => match fs::read_to_string(path.to_string()).await {
                    Ok(casm) => casm.to_string(),
                    Err(e) => e.to_string(),
                },
                None => "".to_string(),
            },
            None => "".to_string(),
        },
        message: String::from_utf8(output.stderr)
            .unwrap()
            .replace(&file_path.to_str().unwrap().to_string(), &remix_file_path)
            .replace(&casm_path.to_str().unwrap().to_string(), &casm_remix_path),
        status: match output.status.code() {
            Some(0) => "Success".to_string(),
            Some(_) => "SierraCompilationFailed".to_string(),
            None => "UnknownError".to_string(),
        },
    })
}

fn get_files_recursive(base_path: &Path) -> Vec<FileContentMap> {
    let mut file_content_map_array: Vec<FileContentMap> = Vec::new();

    if base_path.is_dir() {
        for entry in base_path.read_dir().unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                file_content_map_array.extend(get_files_recursive(&path));
            } else if let Ok(content) = std::fs::read_to_string(&path) {
                let file_name = path.file_name().unwrap().to_string_lossy().to_string();
                let file_content = content;
                let file_content_map = FileContentMap {
                    file_name,
                    file_content,
                };
                file_content_map_array.push(file_content_map);
            }
        }
    }

    file_content_map_array
}

/// Run Scarb to compile a project
///
pub async fn do_scarb_compile(
    remix_file_path: PathBuf,
) -> Result<Json<ScarbCompileResponse>, String> {
    let remix_file_path = match remix_file_path.to_str() {
        Some(path) => path.to_string(),
        None => {
            return Ok(Json(ScarbCompileResponse {
                file_content_map_array: vec![],
                message: "File path not found".to_string(),
                status: "FileNotFound".to_string(),
            }));
        }
    };

    let file_path = get_file_path(&remix_file_path);

    let mut compile = Command::new("scarb");
    compile.current_dir(&file_path);

    let result = compile
        .arg("build")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to execute scarb build");

    println!("LOG: ran command:{:?}", compile);

    let output = result.wait_with_output().expect("Failed to wait on child");

    Ok(Json(ScarbCompileResponse {
        file_content_map_array: get_files_recursive(&file_path.join("target/dev")),
        message: String::from_utf8(output.stdout)
            .unwrap()
            .replace(&file_path.to_str().unwrap().to_string(), &remix_file_path)
            + &String::from_utf8(output.stderr)
                .unwrap()
                .replace(&file_path.to_str().unwrap().to_string(), &remix_file_path),
        status: match output.status.code() {
            Some(0) => "Success".to_string(),
            Some(_) => "SierraCompilationFailed".to_string(),
            None => "UnknownError".to_string(),
        },
    }))
}

/// Run Cairo --version to return Cairo version string
///
pub fn do_cairo_version() -> Result<String, String> {
    let mut version_caller = Command::new("cargo");
    version_caller.current_dir(CAIRO_DIR);
    match String::from_utf8(
        version_caller
            .arg("run")
            .arg("-q")
            .arg("--release")
            .arg("--bin")
            .arg("cairo-compile")
            .arg("--")
            .arg("--version")
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to execute cairo-compile")
            .wait_with_output()
            .expect("Failed to wait on child")
            .stdout,
    ) {
        Ok(version) => Ok(version),
        Err(e) => Err(e.to_string()),
    }
}
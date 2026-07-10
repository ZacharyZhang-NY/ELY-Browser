use std::{
    env,
    error::Error,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let workspace_root = workspace_root(&manifest_dir)?;
    let workspace_manifest_path = workspace_root.join("Cargo.toml");
    let workspace_manifest = read_workspace_manifest(&workspace_manifest_path)?;
    let workspace = table(&workspace_manifest, "workspace")?;
    let package = table(workspace, "package")?;
    let dependencies = table(workspace, "dependencies")?;

    emit_cargo_directive(format!("cargo:rerun-if-changed={}", workspace_manifest_path.display()))?;
    emit_cargo_directive("cargo:rerun-if-env-changed=ELY_BUILD_REVISION")?;

    emit_env("ELY_BUILD_REVISION", &build_revision(workspace_root)?)?;
    emit_env("ELY_WORKSPACE_LICENSE", string_value(package, "license")?)?;
    if env::var("PROFILE").as_deref() == Ok("debug") {
        emit_env("ELY_WORKSPACE_MANIFEST", path_value(&workspace_manifest_path)?)?;
    }
    emit_env("ELY_GPUI_VERSION", dependency_version(dependencies, "gpui")?)?;
    emit_env("ELY_GPUI_COMPONENT_VERSION", dependency_version(dependencies, "gpui-component")?)?;
    emit_env("ELY_SERVO_VERSION", dependency_version(dependencies, "servo")?)?;
    Ok(())
}

fn workspace_root(manifest_dir: &Path) -> Result<&Path, Box<dyn Error>> {
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing crates directory"))?;
    crates_dir
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing workspace root"))
        .map_err(Into::into)
}

fn read_workspace_manifest(path: &Path) -> Result<toml::Table, Box<dyn Error>> {
    let manifest = fs::read_to_string(path)?;
    manifest.parse::<toml::Table>().map_err(Into::into)
}

fn table<'a>(value: &'a toml::Table, key: &str) -> Result<&'a toml::Table, Box<dyn Error>> {
    value
        .get(key)
        .and_then(toml::Value::as_table)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("missing {key} table")))
        .map_err(Into::into)
}

fn string_value<'a>(value: &'a toml::Table, key: &str) -> Result<&'a str, Box<dyn Error>> {
    value
        .get(key)
        .and_then(toml::Value::as_str)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("missing {key} value")))
        .map_err(Into::into)
}

fn path_value(path: &Path) -> Result<&str, Box<dyn Error>> {
    path.to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "workspace path is not UTF-8"))
        .map_err(Into::into)
}

fn dependency_version<'a>(
    dependencies: &'a toml::Table,
    name: &str,
) -> Result<&'a str, Box<dyn Error>> {
    let dependency = dependencies.get(name).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, format!("missing {name} dependency"))
    })?;
    match dependency {
        toml::Value::String(version) => Ok(version),
        toml::Value::Table(table) => string_value(table, "version"),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid {name} dependency version"),
        )
        .into()),
    }
}

fn build_revision(workspace_root: &Path) -> Result<String, Box<dyn Error>> {
    match env::var("ELY_BUILD_REVISION") {
        Ok(revision) => return validated_revision(revision),
        Err(env::VarError::NotUnicode(_)) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ELY_BUILD_REVISION is not UTF-8",
            )
            .into());
        }
        Err(env::VarError::NotPresent) => {}
    }

    match fs::symlink_metadata(workspace_root.join(".git")) {
        Ok(_) => git_revision(workspace_root),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            validated_revision(format!("source-{}", env::var("CARGO_PKG_VERSION")?))
        }
        Err(error) => Err(error.into()),
    }
}

fn git_revision(workspace_root: &Path) -> Result<String, Box<dyn Error>> {
    emit_git_watch(workspace_root, "HEAD")?;
    emit_git_watch(workspace_root, "packed-refs")?;
    if let Some(reference) = git_optional_output(workspace_root, &["symbolic-ref", "-q", "HEAD"])? {
        emit_git_watch(workspace_root, &reference)?;
    }
    let revision = git_output(workspace_root, &["rev-parse", "--short=12", "HEAD"])?;
    validated_revision(revision)
}

fn emit_git_watch(workspace_root: &Path, git_path: &str) -> Result<(), Box<dyn Error>> {
    let path = PathBuf::from(git_output(workspace_root, &["rev-parse", "--git-path", git_path])?);
    let path = if path.is_absolute() { path } else { workspace_root.join(path) };
    emit_cargo_directive(format!("cargo:rerun-if-changed={}", path.display()))
}

fn git_output(workspace_root: &Path, arguments: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = Command::new("git").args(arguments).current_dir(workspace_root).output()?;
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("git {} failed", arguments.join(" ")),
        )
        .into());
    }
    String::from_utf8(output.stdout).map(|value| value.trim().to_string()).map_err(Into::into)
}

fn git_optional_output(
    workspace_root: &Path,
    arguments: &[&str],
) -> Result<Option<String>, Box<dyn Error>> {
    let output = Command::new("git").args(arguments).current_dir(workspace_root).output()?;
    if output.status.success() {
        return String::from_utf8(output.stdout)
            .map(|value| Some(value.trim().to_string()))
            .map_err(Into::into);
    }
    if output.status.code() == Some(1) {
        return Ok(None);
    }
    Err(io::Error::new(io::ErrorKind::InvalidData, format!("git {} failed", arguments.join(" ")))
        .into())
}

fn validated_revision(revision: String) -> Result<String, Box<dyn Error>> {
    let revision = revision.trim();
    validate_env_value("ELY_BUILD_REVISION", revision)?;
    Ok(revision.to_string())
}

fn emit_env(key: &str, value: &str) -> Result<(), Box<dyn Error>> {
    validate_env_value(key, value)?;
    emit_cargo_directive(format!("cargo:rustc-env={key}={value}"))?;
    Ok(())
}

fn validate_env_value(key: &str, value: &str) -> Result<(), Box<dyn Error>> {
    if value.trim().is_empty() || value.contains('\r') || value.contains('\n') {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("invalid {key}")).into());
    }
    Ok(())
}

fn emit_cargo_directive(directive: impl AsRef<str>) -> Result<(), Box<dyn Error>> {
    writeln!(io::stdout(), "{}", directive.as_ref())?;
    Ok(())
}

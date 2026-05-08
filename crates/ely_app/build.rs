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
    let git_head_path = workspace_root.join(".git/HEAD");
    let workspace_manifest = read_workspace_manifest(&workspace_manifest_path)?;
    let workspace = table(&workspace_manifest, "workspace")?;
    let package = table(workspace, "package")?;
    let dependencies = table(workspace, "dependencies")?;

    emit_cargo_directive(format!("cargo:rerun-if-changed={}", workspace_manifest_path.display()))?;
    emit_cargo_directive(format!("cargo:rerun-if-changed={}", git_head_path.display()))?;
    if let Some(ref_path) = git_head_ref(&git_head_path)? {
        emit_cargo_directive(format!(
            "cargo:rerun-if-changed={}",
            workspace_root.join(".git").join(ref_path).display()
        ))?;
    }

    emit_env("ELY_BUILD_REVISION", &git_revision(workspace_root)?)?;
    emit_env("ELY_WORKSPACE_LICENSE", string_value(package, "license")?)?;
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

fn git_revision(workspace_root: &Path) -> Result<String, Box<dyn Error>> {
    let output = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .current_dir(workspace_root)
        .output()?;
    if !output.status.success() {
        return Err(
            io::Error::new(io::ErrorKind::InvalidData, "git revision is unavailable").into()
        );
    }
    let revision = String::from_utf8(output.stdout)?.trim().to_string();
    if revision.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "git revision is empty").into());
    }
    Ok(revision)
}

fn git_head_ref(git_head_path: &Path) -> Result<Option<String>, Box<dyn Error>> {
    let head = fs::read_to_string(git_head_path)?;
    let Some(ref_path) = head.trim().strip_prefix("ref: ") else {
        return Ok(None);
    };
    Ok(Some(ref_path.to_string()))
}

fn emit_env(key: &str, value: &str) -> Result<(), Box<dyn Error>> {
    if value.trim().is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("{key} is empty")).into());
    }
    emit_cargo_directive(format!("cargo:rustc-env={key}={value}"))?;
    Ok(())
}

fn emit_cargo_directive(directive: impl AsRef<str>) -> Result<(), Box<dyn Error>> {
    writeln!(io::stdout(), "{}", directive.as_ref())?;
    Ok(())
}

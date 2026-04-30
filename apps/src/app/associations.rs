//! File associations and desktop integration.

pub fn register_file_associations() {
    #[cfg(target_os = "linux")]
    {
        std::thread::spawn(|| {
            if let Err(err) = register_linux_associations() {
                eprintln!("Failed to register Linux file associations: {err}");
            }
        });
    }

    #[cfg(target_os = "windows")]
    {
        std::thread::spawn(|| {
            if let Err(err) = register_windows_associations() {
                eprintln!("Failed to register Windows file associations: {err}");
            }
        });
    }
}

#[cfg(target_os = "linux")]
fn register_linux_associations() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    let data_dir = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    let applications_dir = data_dir.join("applications");
    let mime_dir = data_dir.join("mime");
    let mime_packages_dir = mime_dir.join("packages");
    let icon_dir = data_dir
        .join("icons")
        .join("hicolor")
        .join("256x256")
        .join("apps");

    fs::create_dir_all(&applications_dir)?;
    fs::create_dir_all(&mime_packages_dir)?;
    fs::create_dir_all(&icon_dir)?;

    let icon_path = icon_dir.join("snappix.png");
    if !icon_path.exists() {
        fs::write(&icon_path, APP_ICON_PNG)?;
    }

    let exec_path = std::env::current_exe()
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| "snappix".to_string());

    let desktop_entry = format!(
        "[Desktop Entry]\n\
Type=Application\n\
Name=Snappix\n\
Exec={} %f\n\
Icon=snappix\n\
MimeType=application/x-snappix-project;\n\
Categories=Development;Graphics;\n",
        exec_path
    );
    let desktop_path = applications_dir.join("snappix.desktop");
    fs::write(&desktop_path, desktop_entry)?;

    let mime_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="application/x-snappix-project">
    <comment>Snappix Project</comment>
    <glob pattern="*.spx"/>
  </mime-type>
</mime-info>
"#;
    let mime_path = mime_packages_dir.join("snappix.xml");
    fs::write(&mime_path, mime_xml)?;

    let _ = Command::new("update-mime-database").arg(&mime_dir).status();
    let _ = Command::new("update-desktop-database")
        .arg(&applications_dir)
        .status();
    let _ = Command::new("xdg-mime")
        .arg("default")
        .arg("snappix.desktop")
        .arg("application/x-snappix-project")
        .status();

    Ok(())
}

#[cfg(target_os = "windows")]
fn register_windows_associations() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::path::PathBuf;
    use winreg::enums::*;
    use winreg::RegKey;

    let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    let base_dir = PathBuf::from(local_app_data).join("Snappix");
    fs::create_dir_all(&base_dir)?;

    let icon_path = base_dir.join("snappix.ico");
    if !icon_path.exists() {
        fs::write(&icon_path, APP_ICON_ICO)?;
    }

    let exe_path = std::env::current_exe()
        .ok()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| "snappix.exe".to_string());

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (classes_key, _) = hkcu.create_subkey("Software\\Classes")?;
    let (ext_key, _) = classes_key.create_subkey(".spx")?;
    ext_key.set_value("", &"Snappix.Project")?;

    let (project_key, _) = classes_key.create_subkey("Snappix.Project")?;
    project_key.set_value("", &"Snappix Project")?;

    let (icon_key, _) = project_key.create_subkey("DefaultIcon")?;
    icon_key.set_value("", &icon_path.to_string_lossy().to_string())?;

    let (command_key, _) = project_key.create_subkey("shell\\open\\command")?;
    command_key.set_value("", &format!("\"{}\" \"%1\"", exe_path))?;

    Ok(())
}

#[cfg(target_os = "linux")]
const APP_ICON_PNG: &[u8] = include_bytes!("../../resources/icons/icon.png");
#[cfg(target_os = "windows")]
const APP_ICON_ICO: &[u8] = include_bytes!("../../resources/icons/icon.ico");

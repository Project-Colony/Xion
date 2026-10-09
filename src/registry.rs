//! Windows registry integration for Xion.
//!
//! Provides `--register` and `--unregister` CLI commands to add or remove
//! Xion from the Windows shell context menu for folders and drives.

use std::io;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

use crate::platform::{CREATE_NO_WINDOW, system_binary};

/// The registry paths we create for context menu integration.
const DIR_SHELL_KEY: &str = r"HKEY_CURRENT_USER\Software\Classes\Directory\shell\Xion";
const DIR_COMMAND_KEY: &str = r"HKEY_CURRENT_USER\Software\Classes\Directory\shell\Xion\command";
const BG_SHELL_KEY: &str = r"HKEY_CURRENT_USER\Software\Classes\Directory\Background\shell\Xion";
const BG_COMMAND_KEY: &str =
    r"HKEY_CURRENT_USER\Software\Classes\Directory\Background\shell\Xion\command";
const DRIVE_SHELL_KEY: &str = r"HKEY_CURRENT_USER\Software\Classes\Drive\shell\Xion";
const DRIVE_COMMAND_KEY: &str = r"HKEY_CURRENT_USER\Software\Classes\Drive\shell\Xion\command";

/// Register Xion in the Windows context menu.
///
/// Adds "Ouvrir avec Xion" entries for:
/// - Right-click on a folder
/// - Right-click on folder background (empty space)
/// - Right-click on a drive
pub fn register() -> io::Result<()> {
    let exe_path = std::env::current_exe()?;
    let exe = exe_path.display().to_string();

    // Folder context menu: right-click on a folder
    reg_add(DIR_SHELL_KEY, "(Default)", "Ouvrir avec Xion")?;
    reg_add(DIR_SHELL_KEY, "Icon", &exe)?;
    reg_add(DIR_COMMAND_KEY, "(Default)", &format!("\"{}\" \"%1\"", exe))?;

    // Background context menu: right-click on empty space inside a folder
    reg_add(BG_SHELL_KEY, "(Default)", "Ouvrir avec Xion")?;
    reg_add(BG_SHELL_KEY, "Icon", &exe)?;
    reg_add(BG_COMMAND_KEY, "(Default)", &format!("\"{}\" \"%V\"", exe))?;

    // Drive context menu: right-click on a drive
    reg_add(DRIVE_SHELL_KEY, "(Default)", "Ouvrir avec Xion")?;
    reg_add(DRIVE_SHELL_KEY, "Icon", &exe)?;
    reg_add(
        DRIVE_COMMAND_KEY,
        "(Default)",
        &format!("\"{}\" \"%1\"", exe),
    )?;

    println!("Xion enregistré dans le menu contextuel Windows.");
    Ok(())
}

/// Remove Xion from the Windows context menu.
pub fn unregister() -> io::Result<()> {
    reg_delete(DIR_SHELL_KEY)?;
    reg_delete(BG_SHELL_KEY)?;
    reg_delete(DRIVE_SHELL_KEY)?;

    println!("Xion retiré du menu contextuel Windows.");
    Ok(())
}

/// `reg.exe` by its absolute System32 path, silent and without a console window.
///
/// Never by bare name: `--register` usually runs from the folder the exe was
/// downloaded to, and a bare name would run a `reg.exe` planted next to it
/// (see [`system_binary`]).
fn reg_command() -> Command {
    let mut command = Command::new(system_binary("reg.exe"));
    command
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn reg_add(key: &str, value_name: &str, data: &str) -> io::Result<()> {
    let mut args = vec!["add", key];

    // Use /ve for the default (unnamed) value, /v for named values
    if value_name == "(Default)" {
        args.extend(["/ve", "/t", "REG_SZ", "/d", data, "/f"]);
    } else {
        args.extend(["/v", value_name, "/t", "REG_SZ", "/d", data, "/f"]);
    }

    let status = reg_command().args(&args).status()?;

    if !status.success() {
        return Err(io::Error::other(format!("Échec de reg add pour {}", key)));
    }
    Ok(())
}

fn reg_delete(key: &str) -> io::Result<()> {
    let status = reg_command().args(["delete", key, "/f"]).status()?;

    if !status.success() {
        // Key might not exist — not an error
        tracing::warn!("Clé registre absente ou déjà supprimée : {}", key);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reg_runs_from_system32() {
        let command = reg_command();
        let program = std::path::Path::new(command.get_program());
        assert!(
            program.is_absolute(),
            "reg.exe must not be resolved by bare name"
        );
        assert!(program.ends_with(r"System32\reg.exe"));
    }
}

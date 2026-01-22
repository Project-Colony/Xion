use xion::core::AppConfig;
use xion::filesystem::{FileSystem, ListOptions, LocalFileSystem};
use xion::ui::AppState;

fn main() {
    let config = AppConfig::default();
    let state = AppState::new(config.clone());
    let filesystem = LocalFileSystem::new();
    let options = ListOptions {
        show_hidden: config.show_hidden,
    };

    println!("Xion prototype - listing {}", state.route.path.display());

    match filesystem.list_dir(&state.route.path, options) {
        Ok(entries) => {
            for entry in entries {
                println!("{} ({:?})", entry.name, entry.entry_type);
            }
        }
        Err(error) => {
            eprintln!("Failed to list directory: {error}");
        }
    }
}

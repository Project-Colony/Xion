//! Quel interpréteur lance le terminal intégré, et sous quel nom.
//!
//! Le sélecteur affichait les mêmes trois boutons sur les deux systèmes —
//! `CMD`, `PS`, `Bash` — parce que l'énumération avait été écrite en pensant
//! Windows. Sous Linux cela donnait un bouton « CMD » qui lançait en réalité le
//! `$SHELL` de l'utilisateur, donc juste mal nommé, et un bouton « PS » qui ne
//! lançait rien du tout : le code appelle `powershell`, or même PowerShell
//! installé sous Linux s'appelle `pwsh`.
//!
//! Ici la liste est **découverte** au moment de l'affichage, et la variante par
//! défaut porte enfin le nom de ce qu'elle fait.

use crate::core::ShellConfig;

/// Une entrée du sélecteur : ce qui s'affiche, et ce que ça lance.
pub(super) struct ShellChoice {
    pub(super) label: String,
    pub(super) config: ShellConfig,
}

/// Le programme à lancer et ses arguments.
///
/// Ne démarre rien : c'est `iced_term` qui ouvre le pseudo-terminal, et il
/// attend un nom de programme. Xion se contente de dire lequel.
pub(super) fn resolve(shell: &ShellConfig) -> std::io::Result<(String, Vec<String>)> {
    let resolved = match shell {
        ShellConfig::System => (system_shell(), Vec::new()),
        ShellConfig::PowerShell => (
            find_powershell(),
            vec!["-NoLogo".to_string(), "-NoProfile".to_string()],
        ),
        ShellConfig::Bash => (find_bash(), Vec::new()),
        ShellConfig::Custom(path) => {
            // Un shell personnalisé est un chemin arbitraire lu dans
            // config.toml : il est résolu et vérifié avant d'être transmis.
            (validated_custom_shell(path)?, Vec::new())
        }
    };
    Ok(resolved)
}

/// Les interpréteurs proposés sur cette machine, dans l'ordre d'affichage.
///
/// Le premier est toujours celui de l'utilisateur ; les suivants n'apparaissent
/// que s'ils existent réellement. Un bouton qui ne peut rien lancer ne rend
/// service à personne — il occupe la place et fait douter du reste.
#[cfg(not(windows))]
pub(super) fn available_shells() -> Vec<ShellChoice> {
    let system = system_shell();
    let mut choices = vec![ShellChoice {
        label: display_name(&system),
        config: ShellConfig::System,
    }];

    // Bash en second, sauf quand c'est déjà le shell de connexion : deux
    // boutons pour le même programme se lisent comme deux choix.
    if let Some(bash) = bash_path()
        && display_name(&bash) != display_name(&system)
    {
        choices.push(ShellChoice {
            label: display_name(&bash),
            config: ShellConfig::Bash,
        });
    }

    choices
}

/// Voir la version Unix. Sous Windows les trois interpréteurs sont des
/// programmes distincts, et le choix a un sens que le nom seul ne rend pas.
#[cfg(windows)]
pub(super) fn available_shells() -> Vec<ShellChoice> {
    let mut choices = vec![
        ShellChoice {
            label: display_name(&system_shell()),
            config: ShellConfig::System,
        },
        ShellChoice {
            label: "PowerShell".to_string(),
            config: ShellConfig::PowerShell,
        },
    ];

    // Git Bash n'est pas livré avec Windows : ne l'annoncer que s'il est là.
    if bash_path().is_some() {
        choices.push(ShellChoice {
            label: "Git Bash".to_string(),
            config: ShellConfig::Bash,
        });
    }

    choices
}

/// Le nom affiché d'une configuration, pour les préférences comme pour le
/// sélecteur.
pub(super) fn label_for(shell: &ShellConfig) -> String {
    match shell {
        ShellConfig::System => display_name(&system_shell()),
        ShellConfig::PowerShell => "PowerShell".to_string(),
        ShellConfig::Bash => display_name(&find_bash()),
        ShellConfig::Custom(path) => display_name(path),
    }
}

/// Le shell de connexion de l'utilisateur.
///
/// `$SHELL` d'abord, parce que c'est la convention et que la session la pose.
/// Mais elle peut manquer selon la façon dont Xion est lancé, et retomber
/// directement sur `/bin/sh` donnerait un shell que l'utilisateur n'a pas
/// choisi alors que le système, lui, sait lequel c'est.
#[cfg(not(windows))]
fn system_shell() -> String {
    if let Some(shell) = std::env::var_os("SHELL") {
        let shell = shell.to_string_lossy().into_owned();
        if !shell.trim().is_empty() {
            return shell;
        }
    }

    shell_from_passwd().unwrap_or_else(|| "/bin/sh".to_string())
}

#[cfg(windows)]
fn system_shell() -> String {
    std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_string())
}

/// Le shell inscrit dans la base des comptes, quand `$SHELL` est muette.
///
/// Lit `/etc/passwd` plutôt que d'appeler `getpwuid_r` : cela évite un bloc
/// `unsafe` et une dépendance directe, et surtout cela rend l'analyse
/// testable. La contrepartie est un utilisateur déclaré ailleurs — LDAP,
/// systemd-homed — qu'on ne verra pas ; mais celui-là a une session qui pose
/// `$SHELL`, donc il n'arrive jamais jusqu'ici.
#[cfg(target_os = "linux")]
fn shell_from_passwd() -> Option<String> {
    use std::os::unix::fs::MetadataExt;

    // `/proc/self` appartient à l'uid réel du processus. Le lire évite de
    // dépendre de `$USER`, qui peut manquer exactement quand `$SHELL` manque.
    let uid = std::fs::metadata("/proc/self").ok()?.uid();
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    shell_for_uid(&passwd, uid)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn shell_from_passwd() -> Option<String> {
    None
}

/// Extrait le shell de l'uid donné d'un contenu de `/etc/passwd`.
///
/// Séparée de la lecture du fichier pour être exerçable sans en fabriquer un.
#[cfg(target_os = "linux")]
fn shell_for_uid(passwd: &str, uid: u32) -> Option<String> {
    // nom:mot_de_passe:uid:gid:commentaire:accueil:shell
    const UID: usize = 2;
    const SHELL: usize = 6;

    for line in passwd.lines() {
        // Indexer plutôt que d'avancer un itérateur : la première écriture
        // enchaînait deux `nth(2)`, ce qui atterrissait sur le dossier
        // personnel et non sur le shell. Et son `?` sortait de la fonction
        // entière à la première ligne malformée, au lieu de la sauter.
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() <= SHELL {
            continue;
        }
        if fields[UID].parse::<u32>() != Ok(uid) {
            continue;
        }
        let shell = fields[SHELL].trim();
        if !shell.is_empty() {
            return Some(shell.to_string());
        }
    }
    None
}

/// Le nom court d'un interpréteur : `/usr/bin/zsh` devient `zsh`.
fn display_name(path: &str) -> String {
    let name = std::path::Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());

    // `cmd.exe` se lit mieux en `cmd`, et l'extension n'apprend rien.
    match name.rfind('.') {
        Some(dot) if name[dot..].eq_ignore_ascii_case(".exe") => name[..dot].to_string(),
        _ => name,
    }
}

/// Resolve a user-configured shell path, refusing anything that is not an
/// existing absolute path.
///
/// A bare name would be resolved by the OS, and on Windows that search visits
/// the directory of the running executable before the system one.
fn validated_custom_shell(path: &str) -> std::io::Result<String> {
    let candidate = std::path::Path::new(path);
    if !candidate.is_absolute() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("terminal_shell doit être un chemin absolu : {path}"),
        ));
    }
    let resolved = candidate.canonicalize()?;
    if !resolved.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("terminal_shell introuvable : {path}"),
        ));
    }
    Ok(resolved.to_string_lossy().into_owned())
}

/// PowerShell, par son chemin canonique quand il est connu.
///
/// Pas le nom nu : sous Windows la recherche visite le dossier de l'exécutable
/// en cours avant celui du système, ce qui est le même piège que pour un shell
/// personnalisé. `pwsh` d'abord, c'est la version maintenue.
fn find_powershell() -> String {
    #[cfg(windows)]
    {
        if let Ok(root) = std::env::var("SystemRoot") {
            let canonical =
                std::path::Path::new(&root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
            if canonical.is_file() {
                return canonical.to_string_lossy().into_owned();
            }
        }
    }

    for candidate in [
        "/usr/bin/pwsh",
        "/usr/local/bin/pwsh",
        "/opt/microsoft/powershell/7/pwsh",
    ] {
        if std::path::Path::new(candidate).is_file() {
            return candidate.to_string();
        }
    }

    // Dernier recours : laisser le système résoudre et rapporter une erreur
    // claire s'il n'y parvient pas.
    "pwsh".to_string()
}

/// Où se trouve bash, ou `None` s'il n'est pas installé.
///
/// The hard-coded `C:\\Program Files\\Git` pair missed scoop, winget and
/// per-user installs, and the `where bash` fallback returned
/// `C:\\Windows\\System32\\bash.exe` — the WSL launcher, not Git Bash — on any
/// machine with WSL enabled.
pub(super) fn bash_path() -> Option<String> {
    if let Ok(env_path) = std::env::var("GIT_BASH")
        && std::path::Path::new(&env_path).is_file()
    {
        return Some(env_path);
    }

    #[cfg(windows)]
    {
        let roots = [
            "ProgramFiles",
            "ProgramW6432",
            "ProgramFiles(x86)",
            "LOCALAPPDATA",
        ];
        for root in roots {
            let Ok(base) = std::env::var(root) else {
                continue;
            };
            for suffix in [r"Git\bin\bash.exe", r"Programs\Git\bin\bash.exe"] {
                let candidate = std::path::Path::new(&base).join(suffix);
                if candidate.is_file() {
                    return Some(candidate.to_string_lossy().into_owned());
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        for candidate in ["/bin/bash", "/usr/bin/bash", "/usr/local/bin/bash"] {
            if std::path::Path::new(candidate).is_file() {
                return Some(candidate.to_string());
            }
        }
    }

    None
}

/// Bash, avec un repli qui laisse le système trancher et échouer lisiblement.
pub(super) fn find_bash() -> String {
    bash_path().unwrap_or_else(|| "bash".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shell_shows_under_its_short_name() {
        assert_eq!(display_name("/usr/bin/zsh"), "zsh");
        assert_eq!(display_name("/bin/bash"), "bash");
        assert_eq!(display_name("fish"), "fish");
    }

    /// `cmd.exe` affiché tel quel ferait le seul bouton à porter une extension.
    ///
    /// Sur le nom seul, pas sur un chemin Windows complet : `Path` ne traite la
    /// barre inversée comme un séparateur que *sur* Windows, donc une telle
    /// assertion échouerait ici pour une raison sans rapport avec ce qu'elle
    /// prétend vérifier.
    #[test]
    fn a_windows_executable_loses_its_extension() {
        assert_eq!(display_name("cmd.exe"), "cmd");
        assert_eq!(display_name("PowerShell.EXE"), "PowerShell");
    }

    /// Un point qui n'est pas une extension Windows ne doit rien couper.
    #[test]
    fn a_dot_that_is_not_an_extension_is_kept() {
        assert_eq!(display_name("/usr/bin/python3.11"), "python3.11");
    }

    #[test]
    fn the_label_of_a_custom_shell_is_its_file_name() {
        assert_eq!(
            label_for(&ShellConfig::Custom("/usr/bin/fish".to_string())),
            "fish"
        );
        assert_eq!(label_for(&ShellConfig::PowerShell), "PowerShell");
    }

    /// Le premier choix proposé est toujours celui de l'utilisateur.
    #[test]
    fn the_first_choice_is_the_users_own_shell() {
        let choices = available_shells();
        assert!(!choices.is_empty());
        assert_eq!(choices[0].config, ShellConfig::System);
    }

    /// Chaque interpréteur proposé doit désigner un exécutable qui existe.
    ///
    /// C'est le test qui manquait à la version précédente : le bouton « PS »
    /// s'affichait sous Linux en ne pouvant rien lancer, et rien ne le disait.
    /// Proposer un choix, c'est promettre qu'il fonctionne.
    #[test]
    fn every_offered_shell_resolves_to_a_real_program() {
        for choice in available_shells() {
            let resolved = resolve(&choice.config);
            let (program, _) = resolved.expect("un interpréteur proposé doit se résoudre");
            assert!(
                std::path::Path::new(&program).is_file(),
                "« {} » est proposé mais {program} n'existe pas",
                choice.label
            );
        }
    }

    /// Aucune entrée ne doit se répéter, sans quoi le même programme aurait
    /// deux boutons — le cas de quelqu'un dont le shell de connexion est bash.
    #[test]
    fn no_two_choices_carry_the_same_label() {
        let choices = available_shells();
        let mut labels: Vec<&str> = choices.iter().map(|c| c.label.as_str()).collect();
        labels.sort_unstable();
        let before = labels.len();
        labels.dedup();
        assert_eq!(before, labels.len(), "un interpréteur proposé deux fois");
    }

    #[cfg(target_os = "linux")]
    mod passwd {
        use super::*;

        const SAMPLE: &str = "root:x:0:0:root:/root:/bin/bash\n\
                              alice:x:1000:1000:Alice:/home/alice:/usr/bin/zsh\n\
                              bob:x:1001:1001:Bob:/home/bob:/usr/bin/fish\n";

        #[test]
        fn the_shell_of_the_matching_uid_is_returned() {
            assert_eq!(shell_for_uid(SAMPLE, 1000).as_deref(), Some("/usr/bin/zsh"));
            assert_eq!(
                shell_for_uid(SAMPLE, 1001).as_deref(),
                Some("/usr/bin/fish")
            );
            assert_eq!(shell_for_uid(SAMPLE, 0).as_deref(), Some("/bin/bash"));
        }

        #[test]
        fn an_unknown_uid_yields_nothing() {
            assert_eq!(shell_for_uid(SAMPLE, 4242), None);
        }

        /// Un compte système sans shell déclaré ne doit pas produire une chaîne
        /// vide, qui serait ensuite lancée comme un programme.
        #[test]
        fn an_empty_shell_field_is_not_a_shell() {
            assert_eq!(shell_for_uid("daemon:x:2:2:daemon:/sbin:\n", 2), None);
        }

        /// Une ligne tronquée ne doit pas faire paniquer l'analyse.
        #[test]
        fn a_malformed_line_is_skipped() {
            assert_eq!(
                shell_for_uid("cassé\nalice:x:1000:1000::/home:/bin/sh\n", 1000).as_deref(),
                Some("/bin/sh")
            );
            assert_eq!(shell_for_uid("a:x:pas-un-nombre:0::/:/bin/sh\n", 0), None);
        }
    }
}

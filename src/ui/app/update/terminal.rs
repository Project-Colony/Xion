//! The integrated terminal panel.
//!
//! L'émulation appartient à `iced_term`, adossé à `alacritty_terminal` : grille
//! de cellules, curseur en deux dimensions, écran alterné, frappes transmises
//! telles quelles. Ce module ne fait plus que décider *quand* un émulateur
//! naît, meurt, et vers quel onglet router ce qu'il produit.
//!
//! Ce qui a disparu avec l'ancien modèle : la ligne de saisie en bas du
//! panneau, la complétion par Tab qu'elle imposait, et l'interrogation
//! périodique toutes les 50 ms. Le shell fait sa propre complétion, et le
//! pseudo-terminal signale lui-même qu'il a écrit.

use std::sync::atomic::{AtomicU64, Ordering};

use iced::Task;

use crate::ui::UiMessage;

use crate::ui::app::types::*;

use super::Flow;
use crate::ui::app::XionApp;

/// Identifiants d'émulateur, distincts pour toute la vie du programme.
///
/// `iced_term` s'en sert pour appairer une souscription à son terminal. L'index
/// de l'onglet ne conviendrait pas : fermer un onglet décale les suivants, et
/// les événements en vol se retrouveraient livrés au mauvais.
static NEXT_TERMINAL_ID: AtomicU64 = AtomicU64::new(0);

impl XionApp {
    /// Returns `Err(message)` when the message belongs to another domain,
    /// handing it to the next handler in the chain.
    pub(super) fn update_terminal(
        &mut self,
        message: UiMessage,
        tasks: &mut Vec<Task<UiMessage>>,
    ) -> Result<Flow, UiMessage> {
        match message {
            UiMessage::ToggleTerminal => {
                if self.terminal_anim_target > 0.5 {
                    // Fermeture : les émulateurs partent, et leurs shells avec.
                    for tab in &mut self.terminal.tabs {
                        tab.terminal = None;
                    }
                    self.terminal_anim_target = 0.0;
                } else {
                    self.terminal_anim_target = 1.0;
                    let cwd = self.current_directory();
                    self.terminal.active().cwd = Some(cwd);
                    self.start_active_terminal(tasks);
                }
            }
            UiMessage::TerminalEvent(event) => {
                let iced_term::Event::BackendCall(id, command) = event;

                let Some(tab) = self
                    .terminal
                    .tabs
                    .iter_mut()
                    .find(|tab| tab.terminal.as_ref().is_some_and(|term| term.id == id))
                else {
                    // L'onglet a été fermé pendant que l'événement voyageait.
                    return Ok(Flow::Continue);
                };

                let Some(terminal) = tab.terminal.as_mut() else {
                    return Ok(Flow::Continue);
                };

                match terminal.handle(iced_term::Command::ProxyToBackend(command)) {
                    iced_term::actions::Action::Shutdown => {
                        // Le shell a rendu la main : `exit`, ou une mort.
                        tab.terminal = None;
                    }
                    iced_term::actions::Action::ChangeTitle(title) => {
                        // Ce que le programme veut qu'on l'appelle — le shell y
                        // met en général le dossier courant ou la commande.
                        if !title.trim().is_empty() {
                            tab.title = title;
                        }
                    }
                    iced_term::actions::Action::Ignore => {}
                }
            }
            UiMessage::TerminalAddTab => {
                const MAX_TERMINAL_TABS: usize = 10;
                if self.terminal.tabs.len() >= MAX_TERMINAL_TABS {
                    self.last_action =
                        Some(format!("Maximum {MAX_TERMINAL_TABS} onglets terminal"));
                    return Ok(Flow::Stop(Task::none()));
                }
                let count = self.terminal.tabs.len() + 1;
                self.terminal.tabs.push(TerminalTab {
                    title: format!("Terminal {count}"),
                    ..Default::default()
                });
                self.terminal.active_tab = self.terminal.tabs.len() - 1;
                let cwd = self.current_directory();
                self.terminal.active().cwd = Some(cwd);
                self.start_active_terminal(tasks);
            }
            UiMessage::TerminalCloseTab(index) => {
                if self.terminal.tabs.len() > 1 && index < self.terminal.tabs.len() {
                    // Retirer l'onglet détruit son émulateur, donc son shell.
                    self.terminal.tabs.remove(index);
                    if self.terminal.active_tab >= self.terminal.tabs.len() {
                        self.terminal.active_tab = self.terminal.tabs.len() - 1;
                    }
                }
            }
            UiMessage::TerminalSwitchTab(index) => {
                if index < self.terminal.tabs.len() {
                    self.terminal.active_tab = index;
                    // Un onglet ouvert alors que le panneau était replié n'a pas
                    // encore de shell : c'est en y venant qu'il en mérite un.
                    if self.terminal_anim_target > 0.5 {
                        self.start_active_terminal(tasks);
                    }
                }
            }
            UiMessage::TerminalAnimTick => {
                let speed = 0.15;
                let diff = self.terminal_anim_target - self.terminal_anim_progress;
                if diff.abs() < 0.005 {
                    self.terminal_anim_progress = self.terminal_anim_target;
                } else {
                    self.terminal_anim_progress += diff * speed;
                }
            }
            UiMessage::TerminalInterrupt => {
                // `0x03`, l'octet que Ctrl+C envoie. Le raccourci fonctionne
                // désormais tout seul quand le terminal a le focus ; ce bouton
                // reste pour qui ne le connaît pas.
                const ETX: u8 = 0x03;
                match self.terminal.active().terminal.as_mut() {
                    Some(terminal) => {
                        terminal.handle(iced_term::Command::ProxyToBackend(
                            iced_term::BackendCommand::Write(vec![ETX]),
                        ));
                    }
                    None => self
                        .terminal
                        .push_notice("Aucun processus à interrompre.".to_string()),
                }
            }
            UiMessage::SetShell(shell) => {
                self.state.config.terminal_shell = shell;
                // L'émulateur en cours porte l'ancien interpréteur : il part.
                self.terminal.active().terminal = None;
                self.config_manager.save(&self.state.config);

                // Et il faut le remplacer immédiatement. Le commentaire
                // d'origine disait « so next open uses new shell », mais on ne
                // change d'interpréteur que panneau ouvert : il n'y a pas de
                // prochaine ouverture. Le terminal restait donc muet, et la
                // première commande tapée répondait « terminal non démarré »
                // sans que rien n'explique pourquoi.
                if self.terminal_anim_target > 0.5 {
                    self.start_active_terminal(tasks);
                }
            }
            UiMessage::WindowResized(width, height) => {
                self.window_size = (width, height);
            }
            other => return Err(other),
        }

        Ok(Flow::Continue)
    }

    /// Le dossier que le terminal doit ouvrir.
    fn current_directory(&self) -> std::path::PathBuf {
        self.state
            .route
            .local_path()
            .cloned()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    }

    /// Donne un shell à l'onglet actif s'il n'en a pas déjà un.
    ///
    /// Ne fait rien quand un émulateur vit déjà : rouvrir le panneau ou revenir
    /// sur un onglet ne doit pas abandonner le programme qui y tourne.
    fn start_active_terminal(&mut self, tasks: &mut Vec<Task<UiMessage>>) {
        if self
            .terminal
            .active_ref()
            .is_some_and(|tab| tab.terminal.is_some())
        {
            return;
        }

        let fallback = self.current_directory();
        let working_directory = self
            .terminal
            .active_ref()
            .and_then(|tab| tab.cwd.clone())
            .unwrap_or(fallback);

        let (program, args) =
            match crate::ui::app::shell::resolve(&self.state.config.terminal_shell) {
                Ok(resolved) => resolved,
                Err(error) => {
                    self.terminal
                        .push_notice(format!("Interpréteur introuvable : {error}"));
                    return;
                }
            };

        // La police de Xion est déjà à chasse fixe — JetBrains Mono Nerd Font —
        // donc le terminal hérite de la même, et les glyphes Nerd Font que les
        // invites modernes affichent s'y rendent sans police de secours.
        let tokens = crate::ui::theme::UiTokens::for_theme(&self.state.config.theme);

        let settings = iced_term::settings::Settings {
            font: iced_term::settings::FontSettings {
                size: tokens.typography.body,
                font_type: tokens.typography.body_font,
                ..Default::default()
            },
            theme: iced_term::settings::ThemeSettings::new(Box::new(terminal_palette(
                tokens.colors,
            ))),
            backend: iced_term::settings::BackendSettings {
                program,
                args,
                working_directory: Some(working_directory),
                env: terminal_environment(),
            },
        };

        let id = NEXT_TERMINAL_ID.fetch_add(1, Ordering::Relaxed);
        match iced_term::Terminal::new(id, settings) {
            Ok(mut terminal) => {
                // Donner tout de suite une taille plausible, en pixels.
                //
                // `TerminalSize::default()` de la caisse déclare une zone de
                // 80×50 *pixels* avec des cellules de 1×1. À la première
                // synchronisation de police — que `handle` déclenche à chaque
                // commande — la vraie mesure des cellules arrive alors que la
                // zone vaut encore 80×50 pixels, et la grille tombe à
                // `80/8 = 10` colonnes sur `50/18 = 2` lignes. Sur dix colonnes
                // une invite zsh enroule à chaque frappe et zle redessine sans
                // fin : les caractères paraissaient se dupliquer alors que le
                // shell recevait exactement ce qu'on lui envoyait.
                //
                // L'estimation n'a pas besoin d'être juste, seulement d'être du
                // bon ordre : le widget publiera la mesure exacte dès qu'il
                // sera dans l'arbre, et le noyau ne prévient le shell que
                // lorsque les dimensions changent réellement.
                let (window_width, _) = self.window_size;
                terminal.handle(iced_term::Command::ProxyToBackend(
                    iced_term::BackendCommand::Resize(
                        Some(iced::Size {
                            width: window_width.max(200.0),
                            height: crate::ui::theme::layout::TERMINAL_DEFAULT_HEIGHT,
                        }),
                        None,
                    ),
                ));

                let widget_id = terminal.widget_id().clone();
                let tab = self.terminal.active();
                tab.notice = None;
                tab.terminal = Some(terminal);

                // Sans ça il fallait cliquer dans la grille avant que la
                // moindre touche y parvienne : le widget ignore le clavier tant
                // qu'il n'a pas le focus. C'est ce que fait l'exemple officiel
                // de la caisse à chaque création de panneau.
                tasks.push(iced_term::TerminalView::focus(widget_id));
            }
            Err(error) => self
                .terminal
                .push_notice(format!("Le terminal n'a pas démarré : {error}")),
        }
    }
}

/// L'environnement du shell : ce qu'il doit savoir du terminal qui l'héberge.
///
/// **C'est la correction du dédoublement des caractères.** `alacritty_terminal`
/// fournit `setup_env()`, qui pose `TERM` et `COLORTERM` — mais `iced_term` ne
/// l'appelle jamais. Le shell héritait donc du `TERM` de Xion, c'est-à-dire de
/// *rien* quand Xion est lancé depuis le bureau plutôt que depuis un terminal.
///
/// Sans `TERM`, zsh se croit sur un terminal incapable de déplacer son curseur.
/// Pour recolorer sa ligne après chaque frappe, il ne revient donc pas au début :
/// il la réécrit à la suite. D'où `ééccrriiss` pour « écris » — et d'où la
/// sortie de `echo hello`, elle, parfaitement propre : afficher du texte ne
/// demande aucun positionnement.
fn terminal_environment() -> std::collections::HashMap<String, String> {
    let mut env = std::collections::HashMap::new();
    env.insert("TERM".to_string(), terminfo_name().to_string());
    // L'émulateur rend en couleurs vraies ; sans cette variable, les programmes
    // qui la consultent se rabattent sur 256 couleurs.
    env.insert("COLORTERM".to_string(), "truecolor".to_string());
    env
}

/// Le nom terminfo à annoncer.
///
/// Le même choix qu'alacritty : sa propre entrée quand la machine l'a, sinon le
/// repli universel. Annoncer `alacritty` sur un système qui n'a pas l'entrée
/// serait pire que ne rien annoncer — le shell chercherait des capacités
/// introuvables.
fn terminfo_name() -> &'static str {
    if terminfo_exists("alacritty") {
        "alacritty"
    } else {
        "xterm-256color"
    }
}

/// Cherche une entrée terminfo là où la bibliothèque C la chercherait.
///
/// Les entrées sont rangées sous un dossier nommé d'après la première lettre,
/// littérale sur la plupart des systèmes et en hexadécimal sur ceux qui suivent
/// la convention ncurses récente. Les deux sont donc essayées.
fn terminfo_exists(name: &str) -> bool {
    let first = match name.chars().next() {
        Some(letter) => letter,
        None => return false,
    };
    let directories = [format!("{first}"), format!("{:x}", first as usize)];

    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    if let Some(explicit) = std::env::var_os("TERMINFO") {
        roots.push(std::path::PathBuf::from(explicit));
    }
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(std::path::Path::new(&home).join(".terminfo"));
    }
    roots.extend(
        ["/usr/share/terminfo", "/lib/terminfo", "/usr/lib/terminfo"]
            .into_iter()
            .map(std::path::PathBuf::from),
    );

    roots.iter().any(|root| {
        directories
            .iter()
            .any(|directory| root.join(directory).join(name).exists())
    })
}

/// Le fond et le texte du terminal, pris au thème Colony actif.
///
/// Les seize couleurs ANSI gardent celles de `iced_term` : elles décrivent ce
/// que *le programme* demande — « rouge », « vert » — et les redériver depuis
/// une palette d'interface ferait mentir un `ls --color` sur ce qu'il annonce.
/// Le fond et l'avant-plan, eux, appartiennent à la fenêtre.
fn terminal_palette(colors: crate::ui::theme::UiColors) -> iced_term::ColorPalette {
    iced_term::ColorPalette {
        background: hex(colors.panel_background),
        foreground: hex(colors.text_primary),
        ..Default::default()
    }
}

/// `iced_term` attend ses couleurs en texte hexadécimal.
fn hex(color: iced::Color) -> String {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02X}{:02X}{:02X}",
        channel(color.r),
        channel(color.g),
        channel(color.b)
    )
}

#[cfg(test)]
mod tests {
    use super::hex;
    use iced::Color;

    #[test]
    fn a_colour_becomes_the_hex_string_the_emulator_expects() {
        assert_eq!(hex(Color::BLACK), "#000000");
        assert_eq!(hex(Color::WHITE), "#FFFFFF");
        assert_eq!(hex(Color::from_rgb(1.0, 0.0, 0.0)), "#FF0000");
    }

    /// Le shell doit savoir sur quel terminal il parle.
    ///
    /// Sans `TERM`, zsh se croit incapable de déplacer son curseur et réécrit sa
    /// ligne à la suite au lieu de la redessiner : chaque caractère apparaissait
    /// deux fois. La caisse ne pose pas cette variable — c'est à l'hôte de le
    /// faire, et rien ne le rappelle.
    #[test]
    fn the_shell_is_told_which_terminal_it_speaks_to() {
        let env = super::terminal_environment();
        let term = env.get("TERM").expect("TERM doit être transmis au shell");
        assert!(
            !term.trim().is_empty(),
            "un TERM vide vaut un TERM absent pour le shell"
        );
        assert_eq!(env.get("COLORTERM").map(String::as_str), Some("truecolor"));
    }

    /// Et ce nom doit exister dans la base du système, sans quoi le shell
    /// cherche des capacités introuvables — pire que de ne rien annoncer.
    #[test]
    fn the_announced_terminfo_entry_exists_on_this_machine() {
        let name = super::terminfo_name();
        assert!(
            super::terminfo_exists(name),
            "« {name} » est annoncé mais absent de la base terminfo"
        );
    }

    /// Le repli universel doit rester atteignable : c'est lui qui sauve les
    /// machines sans l'entrée d'alacritty.
    #[test]
    fn an_unknown_entry_is_not_claimed_to_exist() {
        assert!(!super::terminfo_exists("ce-terminal-nexiste-pas"));
    }

    /// Une composante hors bornes ne doit pas déborder l'octet.
    ///
    /// Construite par champs plutôt que par `Color::from_rgb`, qui refuse par
    /// assertion ce que ce test veut justement éprouver.
    #[test]
    fn an_out_of_range_channel_is_clamped() {
        let out_of_range = Color {
            r: 2.0,
            g: -1.0,
            b: 0.5,
            a: 1.0,
        };
        assert_eq!(hex(out_of_range), "#FF0080");
    }
}

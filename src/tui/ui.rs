use ratatui::style::{Color, Modifier, Style};

// ---------------------------------------------------------------------------
// Structures de style (inchangées)
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct BasicTableStyle {
    pub header: Style,
    pub body_text: Style,
    pub body_row_highlight: Style,
}

#[derive(Debug)]
pub struct ScrollTableStyle {
    pub table: BasicTableStyle,
    pub col_spacing: usize,
    pub body_col_highlight: Style,
    pub body_cell_highlight: Style,
    pub frozen: BasicTableStyle,
    pub separator: Style,
}

#[derive(Debug)]
pub struct ViewStyle {
    pub scroll_table: ScrollTableStyle,
}

// ---------------------------------------------------------------------------
// Thème "classique" (ton style d'origine, basé sur les 16 couleurs ANSI)
// ---------------------------------------------------------------------------

const CLASSIC_BASIC_TABLE_STYLE: BasicTableStyle = BasicTableStyle {
    header: Style::new().bold().reversed().blue(),
    body_text: Style::new().white(),
    body_row_highlight: Style::new().on_gray().bold(),
};

const CLASSIC_FROZEN_STYLE: BasicTableStyle = BasicTableStyle {
    header: Style::new().bold().reversed().green(),
    body_text: Style::new().fg(Color::Green),
    body_row_highlight: Style::new().on_yellow().bold(),
};

const CLASSIC_SCROLLTABLE_STYLE: ScrollTableStyle = ScrollTableStyle {
    table: CLASSIC_BASIC_TABLE_STYLE,
    col_spacing: 1,
    body_col_highlight: Style::new().red(),
    body_cell_highlight: Style::new().reversed().yellow(),
    frozen: CLASSIC_FROZEN_STYLE,
    separator: Style::new().red(),
};

pub const CLASSIC_VIEWSTYLE: ViewStyle = ViewStyle {
    scroll_table: CLASSIC_SCROLLTABLE_STYLE,
};

// ---------------------------------------------------------------------------
// Palette : 8 couleurs suffisent à décrire un thème complet
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct Palette {
    /// Fond général du tableau (mettre `Color::Reset` pour garder le fond du terminal).
    pub bg: Color,
    /// Couleur du texte courant.
    pub fg: Color,
    /// Couleur principale : fond de l'en-tête.
    pub accent: Color,
    /// Couleur secondaire : colonnes figées (en-tête + texte).
    pub accent2: Color,
    /// Fond de la ligne sélectionnée.
    pub row_hl: Color,
    /// Texte de la colonne sélectionnée.
    pub col_hl: Color,
    /// Fond de la cellule sélectionnée (à l'intersection ligne/colonne).
    pub cell_hl: Color,
    /// Séparateur entre colonnes figées et colonnes mobiles.
    pub separator: Color,
}

/// Construit une couleur RGB depuis un entier hexadécimal : `hex(0xff79c6)`.
const fn hex(rgb: u32) -> Color {
    Color::Rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

impl ViewStyle {
    /// Dérive un `ViewStyle` complet à partir d'une palette.
    pub const fn from_palette(p: Palette) -> Self {
        let table = BasicTableStyle {
            header: Style::new()
                .fg(p.bg)
                .bg(p.accent)
                .add_modifier(Modifier::BOLD),
            body_text: Style::new().fg(p.fg).bg(p.bg),
            body_row_highlight: Style::new().bg(p.row_hl).add_modifier(Modifier::BOLD),
        };

        let frozen = BasicTableStyle {
            header: Style::new()
                .fg(p.bg)
                .bg(p.accent2)
                .add_modifier(Modifier::BOLD),
            body_text: Style::new().fg(p.accent2).bg(p.bg),
            body_row_highlight: Style::new().bg(p.row_hl).add_modifier(Modifier::BOLD),
        };

        ViewStyle {
            scroll_table: ScrollTableStyle {
                table,
                col_spacing: 1,
                body_col_highlight: Style::new().bg(p.col_hl),
                body_cell_highlight: Style::new()
                    .fg(p.bg)
                    .bg(p.cell_hl)
                    .add_modifier(Modifier::BOLD),
                frozen,
                separator: Style::new().fg(p.separator).bg(p.bg),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Les 10 thèmes
// ---------------------------------------------------------------------------

pub const DRACULA: ViewStyle = ViewStyle::from_palette(Palette {
    bg: hex(0x282a36),
    fg: hex(0xf8f8f2),
    accent: hex(0xbd93f9),    // violet
    accent2: hex(0x50fa7b),   // vert
    row_hl: hex(0x44475a),
    col_hl: hex(0x593d52),    // rose désaturé pour un fond
    cell_hl: hex(0xf1fa8c),   // jaune
    separator: hex(0x6272a4), // commentaire
});

pub const NORD: ViewStyle = ViewStyle::from_palette(Palette {
    bg: hex(0x2e3440),
    fg: hex(0xd8dee9),
    accent: hex(0x88c0d0),
    accent2: hex(0xa3be8c),
    row_hl: hex(0x434c5e),
    col_hl: hex(0x3d536b),
    cell_hl: hex(0xebcb8b),
    separator: hex(0x4c566a),
});

pub const GRUVBOX_DARK: ViewStyle = ViewStyle::from_palette(Palette {
    bg: hex(0x282828),
    fg: hex(0xebdbb2),
    accent: hex(0x83a598),
    accent2: hex(0x8ec07c),
    row_hl: hex(0x504945),
    col_hl: hex(0x5a4632),
    cell_hl: hex(0xfabd2f),
    separator: hex(0x928374),
});

pub const SOLARIZED_DARK: ViewStyle = ViewStyle::from_palette(Palette {
    bg: hex(0x002b36),
    fg: hex(0x839496),
    accent: hex(0x268bd2),
    accent2: hex(0x859900),
    row_hl: hex(0x073642),
    col_hl: hex(0x4a3044),
    cell_hl: hex(0xb58900),
    separator: hex(0x586e75),
});

/// Seul thème clair : à réserver aux terminaux avec fond clair (ou si tu laisses `bg` tel quel).
pub const SOLARIZED_LIGHT: ViewStyle = ViewStyle::from_palette(Palette {
    bg: hex(0xfdf6e3),
    fg: hex(0x586e75),
    accent: hex(0x268bd2),
    accent2: hex(0x6c71c4),
    row_hl: hex(0xeee8d5),
    col_hl: hex(0xf2d9e3),
    cell_hl: hex(0xcb4b16),
    separator: hex(0x93a1a1),
});

pub const MONOKAI: ViewStyle = ViewStyle::from_palette(Palette {
    bg: hex(0x272822),
    fg: hex(0xf8f8f2),
    accent: hex(0xa6e22e),
    accent2: hex(0x66d9ef),
    row_hl: hex(0x49483e),
    col_hl: hex(0x59404a),
    cell_hl: hex(0xe6db74),
    separator: hex(0x75715e),
});

pub const TOKYO_NIGHT: ViewStyle = ViewStyle::from_palette(Palette {
    bg: hex(0x1a1b26),
    fg: hex(0xc0caf5),
    accent: hex(0x7aa2f7),
    accent2: hex(0x9ece6a),
    row_hl: hex(0x292e42),
    col_hl: hex(0x443d5e),
    cell_hl: hex(0xe0af68),
    separator: hex(0x414868),
});

pub const CATPPUCCIN_MOCHA: ViewStyle = ViewStyle::from_palette(Palette {
    bg: hex(0x1e1e2e),
    fg: hex(0xcdd6f4),
    accent: hex(0xcba6f7),
    accent2: hex(0xa6e3a1),
    row_hl: hex(0x313244),
    col_hl: hex(0x5a463f),
    cell_hl: hex(0xf9e2af),
    separator: hex(0x585b70),
});

pub const ONE_DARK: ViewStyle = ViewStyle::from_palette(Palette {
    bg: hex(0x282c34),
    fg: hex(0xabb2bf),
    accent: hex(0x61afef),
    accent2: hex(0x98c379),
    row_hl: hex(0x3e4451),
    col_hl: hex(0x4b3d55),
    cell_hl: hex(0xe5c07b),
    separator: hex(0x5c6370),
});

/// Phosphore vert façon terminal rétro / Matrix.
pub const MATRIX: ViewStyle = ViewStyle::from_palette(Palette {
    bg: hex(0x000000),
    fg: hex(0x00cc33),
    accent: hex(0x00ff41),
    accent2: hex(0xb6ffb6),
    row_hl: hex(0x003b00),
    col_hl: hex(0x003b1f),
    cell_hl: hex(0xccffcc),
    separator: hex(0x006600),
});

// ---------------------------------------------------------------------------
// Registre : utile pour un flag `--theme nord` ou une touche qui change de thème
// ---------------------------------------------------------------------------

pub static THEMES: [(&str, ViewStyle); 10] = [
    ("dracula", DRACULA),
    ("nord", NORD),
    ("gruvbox-dark", GRUVBOX_DARK),
    ("solarized-dark", SOLARIZED_DARK),
    ("solarized-light", SOLARIZED_LIGHT),
    ("monokai", MONOKAI),
    ("tokyo-night", TOKYO_NIGHT),
    ("catppuccin-mocha", CATPPUCCIN_MOCHA),
    ("one-dark", ONE_DARK),
    ("matrix", MATRIX),
];

/// Retrouve un thème par son nom (insensible à la casse).
pub fn theme_by_name(name: &str) -> Option<&'static ViewStyle> {
    THEMES
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, style)| style)
}
//! Inline SVG icons on a 24px grid, stroked with `currentColor`.
//!
//! Shapes follow the Lucide icon set (ISC license). Inline SVG keeps every
//! icon crisp at any scale and themable through `color`, with no font or
//! asset loading.

use dioxus::prelude::*;

// The set is shared ahead of the screens that use it. The redesign's
// cleanup pass removes every variant no screen uses, then this allow.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Activity,
    ArrowDown,
    ArrowUp,
    ArrowUpDown,
    Ban,
    Bell,
    Box,
    Braces,
    Calendar,
    CalendarClock,
    ChartColumn,
    Check,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    ChevronsLeft,
    ChevronsRight,
    CircleAlert,
    CircleCheck,
    Clock,
    Code,
    Columns2,
    Columns3,
    Copy,
    CornerDownLeft,
    Database,
    Download,
    Ellipsis,
    ExternalLink,
    Eye,
    FileCode,
    FileText,
    FilePlus,
    Filter,
    Folder,
    FolderOpen,
    FolderPlus,
    GitFork,
    Globe,
    Grid3x3,
    GripVertical,
    Hash,
    History,
    Image,
    Info,
    Key,
    Keyboard,
    Laptop,
    Layers,
    Link,
    List,
    ListX,
    Lock,
    LogOut,
    Map,
    MapPin,
    Maximize,
    Minus,
    Moon,
    Monitor,
    Network,
    Pause,
    Pencil,
    Palette,
    Play,
    Plus,
    RefreshCw,
    RotateCcw,
    Save,
    Search,
    Send,
    Server,
    Settings,
    Shield,
    Square,
    Sun,
    SquareTerminal,
    Table,
    Tablet,
    Trash,
    Unplug,
    TriangleAlert,
    Upload,
    User,
    Users,
    Workflow,
    X,
    Zap,
}

/// One SVG element of an icon.
enum Shape {
    Path(&'static str),
    Circle(f32, f32, f32),
    /// x, y, width, height, corner radius.
    Rect(f32, f32, f32, f32, f32),
    /// cx, cy, rx, ry.
    Ellipse(f32, f32, f32, f32),
}

use Shape::{Circle as C, Ellipse as E, Path as P, Rect as R};

const FOLDER: &str = "M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z";
const FILE: &str = "M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z";
const FILE_FOLD: &str = "M14 2v4a2 2 0 0 0 2 2h4";
const ARC_CCW: &str = "M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8";
const GEAR: &str = "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z";

fn shapes(icon: Icon) -> &'static [Shape] {
    match icon {
        Icon::Activity => &[P(
            "M22 12h-2.48a2 2 0 0 0-1.93 1.46l-2.35 8.36a.25.25 0 0 1-.48 0L9.24 2.18a.25.25 0 0 0-.48 0l-2.35 8.36A2 2 0 0 1 4.49 12H2",
        )],
        Icon::ArrowDown => &[P("M12 5v14"), P("m19 12-7 7-7-7")],
        Icon::ArrowUp => &[P("m5 12 7-7 7 7"), P("M12 19V5")],
        Icon::ArrowUpDown => &[
            P("m21 16-4 4-4-4"),
            P("M17 20V4"),
            P("m3 8 4-4 4 4"),
            P("M7 4v16"),
        ],
        Icon::Ban => &[C(12.0, 12.0, 10.0), P("m4.9 4.9 14.2 14.2")],
        Icon::Bell => &[
            P("M10.268 21a2 2 0 0 0 3.464 0"),
            P(
                "M3.262 15.326A1 1 0 0 0 4 17h16a1 1 0 0 0 .74-1.673C19.41 13.956 18 12.499 18 8A6 6 0 0 0 6 8c0 4.499-1.411 5.956-2.738 7.326",
            ),
        ],
        Icon::Box => &[
            P(
                "M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z",
            ),
            P("m3.3 7 8.7 5 8.7-5"),
            P("M12 22V12"),
        ],
        Icon::Braces => &[
            P("M8 3H7a2 2 0 0 0-2 2v5a2 2 0 0 1-2 2 2 2 0 0 1 2 2v5c0 1.1.9 2 2 2h1"),
            P("M16 21h1a2 2 0 0 0 2-2v-5c0-1.1.9-2 2-2a2 2 0 0 1-2-2V5a2 2 0 0 0-2-2h-1"),
        ],
        Icon::Calendar => &[
            P("M8 2v4"),
            P("M16 2v4"),
            R(3.0, 4.0, 18.0, 18.0, 2.0),
            P("M3 10h18"),
        ],
        Icon::CalendarClock => &[
            P("M21 7.5V6a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h3.5"),
            P("M16 2v4"),
            P("M8 2v4"),
            P("M3 10h5"),
            P("M17.5 17.5 16 16.3V14"),
            C(16.0, 16.0, 6.0),
        ],
        Icon::ChartColumn => &[
            P("M3 3v16a2 2 0 0 0 2 2h16"),
            P("M18 17V9"),
            P("M13 17V5"),
            P("M8 17v-3"),
        ],
        Icon::Check => &[P("M20 6 9 17l-5-5")],
        Icon::ChevronDown => &[P("m6 9 6 6 6-6")],
        Icon::ChevronLeft => &[P("m15 18-6-6 6-6")],
        Icon::ChevronRight => &[P("m9 18 6-6-6-6")],
        Icon::ChevronUp => &[P("m18 15-6-6-6 6")],
        Icon::ChevronsLeft => &[P("m11 17-5-5 5-5"), P("m18 17-5-5 5-5")],
        Icon::ChevronsRight => &[P("m6 17 5-5-5-5"), P("m13 17 5-5-5-5")],
        Icon::CircleAlert => &[C(12.0, 12.0, 10.0), P("M12 8v4"), P("M12 16h.01")],
        Icon::CircleCheck => &[C(12.0, 12.0, 10.0), P("m9 12 2 2 4-4")],
        Icon::Clock => &[C(12.0, 12.0, 10.0), P("M12 6v6l4 2")],
        Icon::Code => &[P("m16 18 6-6-6-6"), P("m8 6-6 6 6 6")],
        Icon::Columns2 => &[R(3.0, 3.0, 18.0, 18.0, 2.0), P("M12 3v18")],
        Icon::Columns3 => &[R(3.0, 3.0, 18.0, 18.0, 2.0), P("M9 3v18"), P("M15 3v18")],
        Icon::CornerDownLeft => &[P("M20 4v7a4 4 0 0 1-4 4H4"), P("m9 10-5 5 5 5")],
        Icon::Copy => &[
            R(8.0, 8.0, 14.0, 14.0, 2.0),
            P("M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"),
        ],
        Icon::Database => &[
            E(12.0, 5.0, 9.0, 3.0),
            P("M3 5V19A9 3 0 0 0 21 19V5"),
            P("M3 12A9 3 0 0 0 21 12"),
        ],
        Icon::Download => &[
            P("M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"),
            P("m7 10 5 5 5-5"),
            P("M12 15V3"),
        ],
        Icon::Ellipsis => &[C(5.0, 12.0, 1.0), C(12.0, 12.0, 1.0), C(19.0, 12.0, 1.0)],
        Icon::ExternalLink => &[
            P("M15 3h6v6"),
            P("M10 14 21 3"),
            P("M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"),
        ],
        Icon::Eye => &[
            P(
                "M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0",
            ),
            C(12.0, 12.0, 3.0),
        ],
        Icon::FileCode => &[
            P("M10 12.5 8 15l2 2.5"),
            P("m14 12.5 2 2.5-2 2.5"),
            P(FILE_FOLD),
            P(FILE),
        ],
        Icon::FilePlus => &[P(FILE), P(FILE_FOLD), P("M9 15h6"), P("M12 18v-6")],
        Icon::FileText => &[
            P(FILE),
            P(FILE_FOLD),
            P("M10 9H8"),
            P("M16 13H8"),
            P("M16 17H8"),
        ],
        Icon::Filter => &[P("M22 3H2l8 9.46V19l4 2v-8.54L22 3z")],
        Icon::Folder => &[P(FOLDER)],
        Icon::FolderOpen => &[P(
            "m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2",
        )],
        Icon::FolderPlus => &[P(FOLDER), P("M12 10v6"), P("M9 13h6")],
        Icon::GitFork => &[
            C(12.0, 18.0, 3.0),
            C(6.0, 6.0, 3.0),
            C(18.0, 6.0, 3.0),
            P("M18 9v2c0 .6-.4 1-1 1H7c-.6 0-1-.4-1-1V9"),
            P("M12 12v3"),
        ],
        Icon::Globe => &[
            C(12.0, 12.0, 10.0),
            P("M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"),
            P("M2 12h20"),
        ],
        Icon::Grid3x3 => &[
            C(5.0, 5.0, 1.5),
            C(12.0, 5.0, 1.5),
            C(19.0, 5.0, 1.5),
            C(5.0, 12.0, 1.5),
            C(12.0, 12.0, 1.5),
            C(19.0, 12.0, 1.5),
            C(5.0, 19.0, 1.5),
            C(12.0, 19.0, 1.5),
            C(19.0, 19.0, 1.5),
        ],
        Icon::GripVertical => &[
            C(9.0, 5.0, 1.0),
            C(15.0, 5.0, 1.0),
            C(9.0, 12.0, 1.0),
            C(15.0, 12.0, 1.0),
            C(9.0, 19.0, 1.0),
            C(15.0, 19.0, 1.0),
        ],
        Icon::Hash => &[
            P("M4 9h16"),
            P("M4 15h16"),
            P("M10 3 8 21"),
            P("M16 3l-2 18"),
        ],
        Icon::History => &[P(ARC_CCW), P("M3 3v5h5"), P("M12 7v5l4 2")],
        Icon::Image => &[
            R(3.0, 3.0, 18.0, 18.0, 2.0),
            C(9.0, 9.0, 2.0),
            P("m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"),
        ],
        Icon::Info => &[C(12.0, 12.0, 10.0), P("M12 16v-4"), P("M12 8h.01")],
        Icon::Key => &[
            P("m15.5 7.5 2.3 2.3a1 1 0 0 0 1.4 0l2.1-2.1a1 1 0 0 0 0-1.4L19 4"),
            P("m21 2-9.6 9.6"),
            C(7.5, 15.5, 5.5),
        ],
        Icon::Keyboard => &[
            R(2.0, 4.0, 20.0, 16.0, 2.0),
            P("M6 8h.01"),
            P("M10 8h.01"),
            P("M14 8h.01"),
            P("M18 8h.01"),
            P("M8 12h.01"),
            P("M12 12h.01"),
            P("M16 12h.01"),
            P("M7 16h10"),
        ],
        Icon::Laptop => &[P(
            "M20 16V7a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v9m16 0H4m16 0 1.28 2.55a1 1 0 0 1-.9 1.45H3.62a1 1 0 0 1-.9-1.45L4 16",
        )],
        Icon::Layers => &[
            P(
                "M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z",
            ),
            P("m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65"),
            P("m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65"),
        ],
        Icon::Link => &[
            P("M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"),
            P("M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"),
        ],
        Icon::List => &[
            P("M3 6h.01"),
            P("M3 12h.01"),
            P("M3 18h.01"),
            P("M8 6h13"),
            P("M8 12h13"),
            P("M8 18h13"),
        ],
        Icon::ListX => &[
            P("M11 12H3"),
            P("M16 6H3"),
            P("M16 18H3"),
            P("m19 10-4 4"),
            P("m15 10 4 4"),
        ],
        Icon::Lock => &[R(3.0, 11.0, 18.0, 11.0, 2.0), P("M7 11V7a5 5 0 0 1 10 0v4")],
        Icon::LogOut => &[
            P("M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"),
            P("m16 17 5-5-5-5"),
            P("M21 12H9"),
        ],
        Icon::Map => &[
            P(
                "M14.106 5.553a2 2 0 0 0 1.788 0l3.659-1.83A1 1 0 0 1 21 4.619v12.764a1 1 0 0 1-.553.894l-4.553 2.277a2 2 0 0 1-1.788 0l-4.212-2.106a2 2 0 0 0-1.788 0l-3.659 1.83A1 1 0 0 1 3 19.381V6.618a1 1 0 0 1 .553-.894l4.553-2.277a2 2 0 0 1 1.788 0z",
            ),
            P("M15 5.764v15"),
            P("M9 3.236v15"),
        ],
        Icon::MapPin => &[
            P(
                "M20 10c0 4.993-5.539 10.193-7.399 11.799a1 1 0 0 1-1.202 0C9.539 20.193 4 14.993 4 10a8 8 0 0 1 16 0",
            ),
            C(12.0, 10.0, 3.0),
        ],
        Icon::Maximize => &[
            P("M8 3H5a2 2 0 0 0-2 2v3"),
            P("M21 8V5a2 2 0 0 0-2-2h-3"),
            P("M3 16v3a2 2 0 0 0 2 2h3"),
            P("M16 21h3a2 2 0 0 0 2-2v-3"),
        ],
        Icon::Minus => &[P("M5 12h14")],
        Icon::Monitor => &[R(2.0, 3.0, 20.0, 14.0, 2.0), P("M8 21h8"), P("M12 17v4")],
        Icon::Moon => &[P("M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z")],
        Icon::Network => &[
            R(16.0, 16.0, 6.0, 6.0, 1.0),
            R(2.0, 16.0, 6.0, 6.0, 1.0),
            R(9.0, 2.0, 6.0, 6.0, 1.0),
            P("M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3"),
            P("M12 12V8"),
        ],
        Icon::Palette => &[
            C(13.5, 6.5, 0.5),
            C(17.5, 10.5, 0.5),
            C(8.5, 7.5, 0.5),
            C(6.5, 12.5, 0.5),
            P(
                "M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.554C21.965 6.012 17.461 2 12 2z",
            ),
        ],
        Icon::Pause => &[R(14.0, 4.0, 4.0, 16.0, 1.0), R(6.0, 4.0, 4.0, 16.0, 1.0)],
        Icon::Pencil => &[
            P(
                "M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z",
            ),
            P("m15 5 4 4"),
        ],
        Icon::Play => &[P("M6 3l14 9-14 9z")],
        Icon::Plus => &[P("M5 12h14"), P("M12 5v14")],
        Icon::RefreshCw => &[
            P("M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"),
            P("M21 3v5h-5"),
            P("M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"),
            P("M8 16H3v5"),
        ],
        Icon::RotateCcw => &[P(ARC_CCW), P("M3 3v5h5")],
        Icon::Save => &[
            P(
                "M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z",
            ),
            P("M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7"),
            P("M7 3v4a1 1 0 0 0 1 1h7"),
        ],
        Icon::Search => &[C(11.0, 11.0, 8.0), P("m21 21-4.3-4.3")],
        Icon::Send => &[
            P(
                "M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z",
            ),
            P("m21.854 2.147-10.94 10.939"),
        ],
        Icon::Server => &[
            R(2.0, 2.0, 20.0, 8.0, 2.0),
            R(2.0, 14.0, 20.0, 8.0, 2.0),
            P("M6 6h.01"),
            P("M6 18h.01"),
        ],
        Icon::Settings => &[P(GEAR), C(12.0, 12.0, 3.0)],
        Icon::Shield => &[P(
            "M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z",
        )],
        Icon::Square => &[R(5.0, 5.0, 14.0, 14.0, 1.5)],
        Icon::Sun => &[
            C(12.0, 12.0, 4.0),
            P("M12 2v2"),
            P("M12 20v2"),
            P("m4.93 4.93 1.41 1.41"),
            P("m17.66 17.66 1.41 1.41"),
            P("M2 12h2"),
            P("M20 12h2"),
            P("m6.34 17.66-1.41 1.41"),
            P("m19.07 4.93-1.41 1.41"),
        ],
        Icon::SquareTerminal => &[
            P("m7 11 2-2-2-2"),
            P("M11 13h4"),
            R(3.0, 3.0, 18.0, 18.0, 2.0),
        ],
        Icon::Table => &[
            P("M12 3v18"),
            R(3.0, 3.0, 18.0, 18.0, 2.0),
            P("M3 9h18"),
            P("M3 15h18"),
        ],
        Icon::Tablet => &[R(4.0, 2.0, 16.0, 20.0, 2.0), P("M12 18h.01")],
        Icon::Trash => &[
            P("M3 6h18"),
            P("M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"),
            P("M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"),
            P("M10 11v6"),
            P("M14 11v6"),
        ],
        Icon::TriangleAlert => &[
            P("m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"),
            P("M12 9v4"),
            P("M12 17h.01"),
        ],
        Icon::Unplug => &[
            P("m19 5 3-3"),
            P("m2 22 3-3"),
            P("M6.3 20.3a2.4 2.4 0 0 0 3.4 0L12 18l-6-6-2.3 2.3a2.4 2.4 0 0 0 0 3.4Z"),
            P("M7.5 13.5 10 11"),
            P("M10.5 16.5 13 14"),
            P("m12 6 6 6 2.3-2.3a2.4 2.4 0 0 0 0-3.4l-2.6-2.6a2.4 2.4 0 0 0-3.4 0Z"),
        ],
        Icon::Upload => &[
            P("M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"),
            P("m17 8-5-5-5 5"),
            P("M12 3v12"),
        ],
        Icon::User => &[
            P("M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"),
            C(12.0, 7.0, 4.0),
        ],
        Icon::Users => &[
            P("M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"),
            C(9.0, 7.0, 4.0),
            P("M22 21v-2a4 4 0 0 0-3-3.87"),
            P("M16 3.13a4 4 0 0 1 0 7.75"),
        ],
        Icon::Workflow => &[
            R(3.0, 3.0, 8.0, 8.0, 2.0),
            P("M7 11v4a2 2 0 0 0 2 2h4"),
            R(13.0, 13.0, 8.0, 8.0, 2.0),
        ],
        Icon::X => &[P("M18 6 6 18"), P("m6 6 12 12")],
        Icon::Zap => &[P(
            "M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z",
        )],
    }
}

/// An icon at `size` CSS pixels (16 by default). Decorative: the control
/// around it carries the accessible label.
#[component]
pub fn Ico(
    icon: Icon,
    #[props(default = 16)] size: u32,
    #[props(default)] class: String,
) -> Element {
    rsx! {
        svg {
            class: "ico {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            for (i, shape) in shapes(icon).iter().enumerate() {
                match shape {
                    Shape::Path(d) => rsx! { path { key: "{i}", d: "{d}" } },
                    Shape::Circle(cx, cy, r) => rsx! { circle { key: "{i}", cx: "{cx}", cy: "{cy}", r: "{r}" } },
                    Shape::Rect(x, y, w, h, rx) => rsx! { rect { key: "{i}", x: "{x}", y: "{y}", width: "{w}", height: "{h}", rx: "{rx}" } },
                    Shape::Ellipse(cx, cy, rx, ry) => rsx! { ellipse { key: "{i}", cx: "{cx}", cy: "{cy}", rx: "{rx}", ry: "{ry}" } },
                }
            }
        }
    }
}

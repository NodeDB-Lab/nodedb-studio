//! Inline SVG icons on a 24px grid, stroked with `currentColor`.
//!
//! Shapes follow the Lucide icon set (ISC license). Inline SVG keeps every
//! icon crisp at any scale and themable through `color`, with no font or
//! asset loading.

use dioxus::prelude::*;

use super::shapes::{Shape, shapes};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
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
    EyeOff,
    FileCode,
    FileText,
    FilePlus,
    Filter,
    Folder,
    FolderOpen,
    FolderPlus,
    GitFork,
    Grid3x3,
    GripVertical,
    Hash,
    Hexagon,
    History,
    Info,
    Key,
    Keyboard,
    Laptop,
    Layers,
    Link,
    List,
    ListX,
    Lock,
    Map,
    MapPin,
    Maximize,
    Minus,
    Moon,
    Monitor,
    MousePointer2,
    Move,
    Network,
    Pause,
    Pencil,
    Palette,
    Play,
    Plus,
    RefreshCw,
    RotateCcw,
    Ruler,
    Save,
    Search,
    Send,
    Settings,
    Shield,
    Square,
    Sun,
    SquareTerminal,
    Table,
    Trash,
    Unplug,
    TriangleAlert,
    Upload,
    User,
    Workflow,
    X,
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

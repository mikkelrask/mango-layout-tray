#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub name: &'static str,
    pub title: &'static str,
    pub symbol: &'static str,
    pub description: &'static str,
}

pub const LAYOUTS: [Layout; 14] = [
    Layout { name: "tile", title: "Tile", symbol: "t", description: "A wide master, a tidy stack." },
    Layout { name: "scroller", title: "Scroller", symbol: "s", description: "A horizontal strip with room to roam." },
    Layout { name: "monocle", title: "Monocle", symbol: "m", description: "One window takes the whole stage." },
    Layout { name: "grid", title: "Grid", symbol: "g", description: "An even grid for an overview." },
    Layout { name: "deck", title: "Deck", symbol: "k", description: "A master beside a stacked deck." },
    Layout { name: "center_tile", title: "Center tile", symbol: "ct", description: "The master sits between two stacks." },
    Layout { name: "vertical_tile", title: "Vertical tile", symbol: "vt", description: "A master above a row of windows." },
    Layout { name: "right_tile", title: "Right tile", symbol: "rt", description: "The master on the right, stack on the left." },
    Layout { name: "vertical_scroller", title: "Vertical scroller", symbol: "vs", description: "A vertical strip you can scroll through." },
    Layout { name: "vertical_grid", title: "Vertical grid", symbol: "vg", description: "An even grid, filled down the columns." },
    Layout { name: "vertical_deck", title: "Vertical deck", symbol: "vk", description: "A master above a stacked deck." },
    Layout { name: "dwindle", title: "Dwindle", symbol: "dw", description: "Recursive splits make a winding mosaic." },
    Layout { name: "fair", title: "Fair", symbol: "f", description: "Balanced columns share the space." },
    Layout { name: "vertical_fair", title: "Vertical fair", symbol: "vf", description: "Balanced rows share the space." },
];

pub fn by_name(name: &str) -> Option<Layout> {
    LAYOUTS.iter().find(|l| l.name == name).copied()
}

pub fn by_symbol(symbol: &str) -> Option<Layout> {
    LAYOUTS.iter().find(|l| l.symbol.eq_ignore_ascii_case(symbol)).copied()
}

pub type Rect = (f64, f64, f64, f64);

pub fn diagram(name: &str) -> Vec<Rect> {
    match name {
        "tile" => vec![(0., 0., 0.56, 1.), (0.56, 0., 0.44, 0.33), (0.56, 0.33, 0.44, 0.34), (0.56, 0.67, 0.44, 0.33)],
        "right_tile" => diagram("tile").into_iter().map(|(x,y,w,h)| (1.-x-w,y,w,h)).collect(),
        "vertical_tile" => diagram("tile").into_iter().map(|(x,y,w,h)| (y,x,h,w)).collect(),
        "center_tile" => vec![(0.23,0.,0.54,1.),(0.,0.,0.23,0.5),(0.,0.5,0.23,0.5),(0.77,0.,0.23,0.5),(0.77,0.5,0.23,0.5)],
        "scroller" => vec![(-0.18,0.,0.4,1.),(0.22,0.,0.56,1.),(0.78,0.,0.4,1.)],
        "vertical_scroller" => diagram("scroller").into_iter().map(|(x,y,w,h)| (y,x,h,w)).collect(),
        "monocle" => vec![(0.,0.,1.,1.)],
        "deck" => vec![(0.,0.,0.56,1.),(0.56,0.,0.36,0.84),(0.60,0.08,0.36,0.84),(0.64,0.16,0.36,0.84)],
        "vertical_deck" => diagram("deck").into_iter().map(|(x,y,w,h)| (y,x,h,w)).collect(),
        "grid" => vec![(0.,0.,0.5,0.5),(0.5,0.,0.5,0.5),(0.,0.5,0.5,0.5),(0.5,0.5,0.5,0.5)],
        "vertical_grid" => vec![(0.,0.,0.5,0.33),(0.,0.33,0.5,0.34),(0.,0.67,0.5,0.33),(0.5,0.,0.5,0.5),(0.5,0.5,0.5,0.5)],
        "dwindle" => vec![(0.,0.,0.5,1.),(0.5,0.,0.5,0.5),(0.75,0.5,0.25,0.5),(0.5,0.75,0.25,0.25),(0.5,0.5,0.25,0.25)],
        "fair" => vec![(0.,0.,0.33,1.),(0.33,0.,0.34,0.5),(0.33,0.5,0.34,0.5),(0.67,0.,0.33,0.5),(0.67,0.5,0.33,0.5)],
        "vertical_fair" => diagram("fair").into_iter().map(|(x,y,w,h)| (y,x,h,w)).collect(),
        _ => vec![],
    }
}

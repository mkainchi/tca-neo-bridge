//! The device diagrams and their hotspots: [key, x, y, diameter] in each SVG's viewBox units, sitting on the
//! numbered callouts. Sidestick: viewBox -76 -8 390 340 (the left-hand setup mirrors 5–10 and 11–16). Quadrant:
//! 533 × 364 (two callouts read 3). NEO: 606 × 726; button 16 exists physically but not in the NEO report.
use crate::mapping::Neo;
use eframe::egui::{self, ColorImage, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use neo_common::config::Direction;
use resvg::{tiny_skia, usvg};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Which way round the sidestick base is set up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hand {
    Left,
    Right,
}

pub struct Diagram {
    pub name: &'static str,
    pub svg: &'static [u8],
    /// viewBox: left, top, width, height.
    pub view: [f32; 4],
    pub spots: &'static [(&'static str, f32, f32, f32)],
}

impl Diagram {
    pub fn aspect(&self) -> f32 {
        self.view[2] / self.view[3]
    }

    /// A hotspot's circle (centre, diameter) inside the plate drawn at `plate`; at least 18 points across.
    pub fn place(&self, plate: Rect, (x, y, size): (f32, f32, f32)) -> (Pos2, f32) {
        let [left, top, width, height] = self.view;
        let centre = plate.min + Vec2::new((x - left) / width * plate.width(), (y - top) / height * plate.height());
        (centre, (size / width * plate.width()).max(18.0))
    }
}

macro_rules! stick {
    ($($n:literal $x:literal $y:literal),*) => { &[$((concat!("s", $n), $x as f32, $y as f32, 27.0)),*] };
}

pub static RIGHT: Diagram = Diagram {
    name: "sidestick-right",
    svg: include_bytes!("../assets/device/sidestick-right.svg"),
    view: [-76.0, -8.0, 390.0, 340.0],
    spots: stick!(5 -48 125, 6 -48 153, 7 -48 181, 10 -48 209, 9 -48 237, 8 -48 265, 11 284 125, 12 284 153, 13 284 181, 16 284 209, 15 284 237, 14 284 265, 1 -48 79, 2 -48 43, 3 -13 98, 4 244 99, 17 260 303),
};

pub static LEFT: Diagram = Diagram {
    name: "sidestick-left",
    svg: include_bytes!("../assets/device/sidestick-left.svg"),
    view: [-76.0, -8.0, 390.0, 340.0],
    spots: stick!(11 -48 125, 12 -48 153, 13 -48 181, 16 -48 209, 15 -48 237, 14 -48 265, 5 284 125, 6 284 153, 7 284 181, 10 284 209, 9 284 237, 8 284 265, 1 -48 79, 2 -48 43, 3 -13 98, 4 244 99, 17 260 303),
};

pub static QUADRANT: Diagram = Diagram {
    name: "quadrant",
    svg: include_bytes!("../assets/device/quadrant.svg"),
    view: [0.0, 0.0, 533.0, 364.0],
    spots: &[("q1", 79.0, 49.0, 30.0), ("q2", 447.0, 65.0, 30.0), ("q3", 473.0, 153.0, 30.0), ("q3", 67.0, 179.0, 30.0), ("q4", 470.0, 204.0, 30.0), ("q5", 58.0, 218.0, 30.0), ("q6", 274.0, 327.0, 30.0), ("q7", 64.0, 308.0, 22.0), ("q8", 86.0, 308.0, 22.0)],
};

pub static NEO: Diagram = Diagram {
    name: "neo",
    svg: include_bytes!("../assets/device/neo.svg"),
    view: [0.0, 0.0, 606.0, 726.0],
    spots: &[
        ("b1", 546.0, 159.0, 34.0), ("b2", 278.0, 185.0, 34.0), ("b3", 427.0, 126.0, 34.0), ("b4", 320.0, 165.0, 34.0),
        ("b5", 262.0, 321.0, 34.0), ("b6", 262.0, 362.0, 34.0), ("b7", 262.0, 402.0, 34.0), ("b8", 262.0, 444.0, 34.0),
        ("b9", 54.0, 218.0, 34.0), ("b10", 54.0, 255.0, 34.0), ("b11", 407.0, 598.0, 34.0), ("b12", 471.0, 598.0, 34.0),
        ("b13", 301.0, 636.0, 34.0), ("b14", 366.0, 636.0, 34.0), ("b15", 329.0, 663.0, 34.0), ("b16", 436.0, 621.0, 34.0),
        ("pov-up", 199.0, 104.0, 18.0), ("pov-right", 222.0, 126.0, 18.0), ("pov-down", 199.0, 147.0, 18.0), ("pov-left", 176.0, 126.0, 18.0),
    ],
};

pub fn stick(hand: Hand) -> &'static Diagram {
    match hand {
        Hand::Left => &LEFT,
        Hand::Right => &RIGHT,
    }
}

/// A NEO hotspot key ("b5", "pov-up") as a target; button 16 is not one (it is not in the NEO report).
pub fn neo_target(key: &str) -> Option<Neo> {
    Some(match key {
        "pov-up" => Neo::Pov(Direction::Up),
        "pov-right" => Neo::Pov(Direction::Right),
        "pov-down" => Neo::Pov(Direction::Down),
        "pov-left" => Neo::Pov(Direction::Left),
        "b16" => return None,
        button => Neo::Button(button[1..].parse().expect("a NEO hotspot")),
    })
}

/// The diagrams' only font: their labels name Arial (regular and bold).
pub const FONTS: [&str; 2] = ["arial.ttf", "arialbd.ttf"];

/// Parsed diagrams, and the latest rendering of each at the size it is shown.
pub struct Art {
    options: usvg::Options<'static>,
    trees: HashMap<&'static str, usvg::Tree>,
    textures: HashMap<&'static str, ([usize; 2], TextureHandle)>,
}

type Shared = Arc<Mutex<Art>>;

fn key() -> egui::Id {
    egui::Id::NULL.with("tca-neo-bridge diagrams")
}

/// Load the diagrams' font from `fonts` (normally `%WINDIR%\Fonts`); a missing font leaves the labels out.
pub fn install(ctx: &egui::Context, fonts: &Path) {
    let mut options = usvg::Options::default();
    for bytes in FONTS.iter().filter_map(|file| std::fs::read(fonts.join(file)).ok()) {
        options.fontdb_mut().load_font_data(bytes);
    }
    let art: Shared = Arc::new(Mutex::new(Art { options, trees: HashMap::new(), textures: HashMap::new() }));
    ctx.data_mut(|data| data.insert_temp(key(), art));
}

/// The diagram rendered for `size` points; it is drawn again only when that size changes.
pub fn texture(ctx: &egui::Context, diagram: &Diagram, size: Vec2) -> TextureHandle {
    let art: Shared = ctx.data(|data| data.get_temp(key())).expect("diagrams installed");
    let mut art = art.lock().expect("diagram art");
    let pixels = (size * ctx.pixels_per_point()).round();
    let wanted = [pixels.x.max(1.0) as usize, pixels.y.max(1.0) as usize];
    if let Some((_, texture)) = art.textures.get(diagram.name).filter(|(rendered, _)| *rendered == wanted) {
        return texture.clone();
    }
    let Art { options, trees, textures } = &mut *art;
    let tree = trees.entry(diagram.name).or_insert_with(|| usvg::Tree::from_data(diagram.svg, options).expect("diagram SVG"));
    let mut pixmap = tiny_skia::Pixmap::new(wanted[0] as u32, wanted[1] as u32).expect("non-empty");
    let scale = tiny_skia::Transform::from_scale(wanted[0] as f32 / tree.size().width(), wanted[1] as f32 / tree.size().height());
    resvg::render(tree, scale, &mut pixmap.as_mut());
    let image = ColorImage::from_rgba_premultiplied(wanted, pixmap.data());
    let texture = ctx.load_texture(diagram.name, image, TextureOptions::LINEAR);
    textures.insert(diagram.name, (wanted, texture.clone()));
    texture
}
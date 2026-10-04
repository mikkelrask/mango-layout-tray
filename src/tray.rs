use crate::{
    Event,
    layout::{LAYOUTS, by_symbol},
};
use async_channel::Sender;
use ksni::{Icon, MenuItem, ToolTip, menu::*};

pub struct Tray {
    pub symbol: String,
    pub monitor: String,
    pub error: Option<String>,
    pub events: Sender<Event>,
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "mango-layout-tray".into()
    }
    fn title(&self) -> String {
        format!("Mango · [{}]", self.symbol.to_lowercase())
    }
    fn activate(&mut self, _: i32, _: i32) {
        let _ = self.events.try_send(Event::Open(true));
    }
    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![icon(&self.symbol, 64), icon(&self.symbol, 32)]
    }
    fn tool_tip(&self) -> ToolTip {
        let name = by_symbol(&self.symbol)
            .map(|l| l.title)
            .unwrap_or("Waiting for MangoWM");
        ToolTip {
            title: "Mango Layout Tray".into(),
            description: self
                .error
                .clone()
                .unwrap_or_else(|| format!("{name} · {}\nClick to choose a layout", self.monitor)),
            ..Default::default()
        }
    }
    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Choose layout…".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.events.try_send(Event::Open(true));
                }),
                ..Default::default()
            }
            .into(),
            SubMenu {
                label: "Quick switch".into(),
                submenu: LAYOUTS
                    .into_iter()
                    .map(|l| {
                        StandardItem {
                            label: format!(
                                "{}{}",
                                if l.symbol.eq_ignore_ascii_case(&self.symbol) {
                                    "✓ "
                                } else {
                                    ""
                                },
                                l.title
                            ),
                            activate: Box::new(move |t: &mut Self| {
                                let _ = t.events.try_send(Event::Quick(l));
                            }),
                            ..Default::default()
                        }
                        .into()
                    })
                    .collect(),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Settings…".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.events.try_send(Event::Settings);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.events.try_send(Event::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn icon(symbol: &str, size: i32) -> Icon {
    use gtk::cairo::{Context, FontSlant, FontWeight, Format, ImageSurface};
    let mut surface = ImageSurface::create(Format::ARgb32, size, size).unwrap();
    {
        let cr = Context::new(&surface).unwrap();
        cr.select_font_face("monospace", FontSlant::Normal, FontWeight::Bold);
        let label = format!("[{}]", symbol.to_lowercase());
        cr.set_font_size(size as f64 * if label.len() > 3 { 0.30 } else { 0.38 });
        let ext = cr.text_extents(&label).unwrap();
        cr.move_to(
            (size as f64 - ext.width()) / 2. - ext.x_bearing(),
            (size as f64 - ext.height()) / 2. - ext.y_bearing(),
        );
        cr.text_path(&label);
        cr.set_source_rgba(0.08, 0.08, 0.08, 0.9);
        cr.set_line_width(size as f64 * 0.045);
        let _ = cr.stroke_preserve();
        cr.set_source_rgb(0.98, 0.98, 0.98);
        let _ = cr.fill();
    }
    let data = surface
        .data()
        .unwrap()
        .chunks_exact(4)
        .flat_map(|pixel| {
            let native = u32::from_ne_bytes(pixel.try_into().unwrap());
            native.to_be_bytes()
        })
        .collect();
    Icon {
        width: size,
        height: size,
        data,
    }
}

//! The client half: the server half sends its status on a channel, this shows it in a corner.
use horizon_mod_sdk::{
    Mod, Registration, chat, client_net,
    hud::{self, Corner, Line},
    browser, command, game, input, log, recipes,
    render::{
        self, CanvasArea, Corner as ShapeCorner, FarTerrain, FillShape, ItemShape, MenuLayout,
        Minimap, PartPose, PlacedModel, Point, Rect, Shape, TextShape, Zoom,
    },
    storage,
    ui::{self, Screen, TextWidget, Widget},
};

#[derive(Default)]
pub struct ShowcaseHud {
    /// `/showcase_tour`: its effects are on.
    touring: bool,
    /// `/showcase_inventory`: the inventory key shows the mod's screen.
    replaced: bool,
    /// `/showcase_far`: far terrain is asked for.
    far: bool,
}

/// The reactor's window: its real slots, with vanilla's cursor and shift-click, where this mod wants them.
fn reactor_layout(charge: f32) {
    let fill = |x, y, w, h, color| Shape::Fill(FillShape { rect: Rect { x, y, w, h }, color });
    let mut shapes = vec![
        fill(0.0, 0.0, 176.0, 170.0, 0xFF_2B_2F_38),
        fill(150.0, 12.0, 16.0, 64.0, 0xFF_10_10_10),
        fill(150.0, 12.0 + 64.0 * (1.0 - charge), 16.0, 64.0 * charge, 0xFF_E0_A0_20),
        Shape::Text(TextShape {
            text: "Reactor".into(),
            at: Point { x: 8.0, y: 6.0 },
            color: 0xFFD040,
            centered: false,
            shadow: true,
        }),
    ];
    let slots = [(30.0, 40.0), (66.0, 22.0), (66.0, 58.0)].map(|(x, y)| Point { x, y });
    for p in &slots {
        shapes.push(fill(p.x, p.y, 16.0, 16.0, 0xFF_8B_8B_8B));
    }
    for i in 0..36 {
        let (x, y) = if i < 27 {
            (8.0 + (i % 9) as f32 * 18.0, 88.0 + (i / 9) as f32 * 18.0)
        } else {
            (8.0 + (i - 27) as f32 * 18.0, 146.0)
        };
        shapes.push(fill(x, y, 16.0, 16.0, 0xFF_55_55_55));
    }
    render::set_menu_layouts(&[MenuLayout {
        title: "Reactor".into(),
        width: 176.0,
        height: 170.0,
        slots: slots.to_vec(),
        inventory: Point { x: 8.0, y: 88.0 },
        shapes,
    }]);
}

/// A machine's window of the mod's own layout: its slots and gauge are shapes, its clicks areas.
fn panel(slots: &str) {
    let (w, h) = render::gui_size();
    let (x, y) = ((w / 2.0 - 88.0).floor(), (h / 2.0 - 60.0).floor());
    let rect = |dx: f32, dy: f32, w: f32, h: f32| Rect { x: x + dx, y: y + dy, w, h };
    let mut lines = slots.lines();
    let mut charge = lines.next().unwrap_or("0 1").split(' ');
    let energy: f32 = charge.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let capacity: f32 = charge.next().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let fill = |rect, color| Shape::Fill(FillShape { rect, color });
    let text = |dx, dy, text: &str, color| {
        Shape::Text(TextShape {
            text: text.to_owned(),
            at: Point { x: x + dx, y: y + dy },
            color,
            centered: false,
            shadow: true,
        })
    };
    let mut shapes = vec![
        fill(rect(0.0, 0.0, 176.0, 100.0), 0xFF_2B_2F_38),
        text(8.0, 6.0, "Reactor", 0xFFD040),
        fill(rect(150.0, 20.0, 16.0, 70.0), 0xFF_10_10_10),
        fill(
            rect(150.0, 20.0 + 70.0 * (1.0 - energy / capacity), 16.0, 70.0 * energy / capacity),
            0xFF_E0_A0_20,
        ),
        text(8.0, 80.0, "Click a slot to take it", 0xA0A0A0),
    ];
    let mut areas = Vec::new();
    let lines = lines.chain(std::iter::repeat("- 0"));
    for (i, line) in lines.take(3).enumerate() {
        let dx = 20.0 + 36.0 * i as f32;
        shapes.push(fill(rect(dx, 30.0, 18.0, 18.0), 0xFF_8B_8B_8B));
        areas.push(CanvasArea { id: i as u32, rect: rect(dx, 30.0, 18.0, 18.0) });
        if let Some((item, count)) = line.split_once(' ')
            && item != "-"
        {
            shapes.push(Shape::Item(ItemShape {
                item: item.to_owned(),
                at: Point { x: x + dx + 1.0, y: y + 31.0 },
                scale: 1.0,
            }));
            shapes.push(text(dx + 10.0, 42.0, count, 0xFFFFFF));
        }
    }
    render::open_canvas("Core Showcase", &shapes, &areas);
}

/// A ring over each reactor, drawn in the world by the game: it needs no entity.
fn rings(list: &str) {
    let models: Vec<PlacedModel> = list
        .lines()
        .filter_map(|line| {
            let mut v = line.split(' ');
            let dimension = v.next()?.to_owned();
            let mut n = || v.next()?.parse::<f64>().ok();
            let (x, y, z) = (n()?, n()?, n()?);
            Some(PlacedModel {
                model: "showcase:ring".into(),
                dimension,
                x: x + 0.5,
                y: y + 1.4,
                z: z + 0.5,
                yaw: 0.0,
                pitch: 0.0,
                roll: 0.0,
                spin: 90.0,
                scale: 1.0,
                // The first part bobs by the mod's hand; "blades" spins in the file.
                parts: vec![PartPose {
                    part: "ring".into(),
                    x: 0.0,
                    y: (y as f32 + x as f32).sin() * 2.0,
                    z: 0.0,
                    pitch: 0.0,
                    yaw: 0.0,
                    roll: 0.0,
                }],
            })
        })
        .collect();
    render::set_models(&models);
}

impl ShowcaseHud {
    /// Everything left of the client's calls: on once, off again the next time.
    fn tour(&mut self) -> String {
        self.touring = !self.touring;
        log::write(log::Level::Info, "showcase tour");
        if !self.touring {
            render::set_zoom(Zoom { factor: 1.0, smooth: true, turn: 1.0 });
            render::set_minimap(None);
            render::clear_corner();
            hud::clear();
            render::set_hud(&[]);
            browser::set_items(&[]);
            storage::delete("tour");
            return "Tour off".into();
        }
        render::shake(1.0, 2.0);
        render::set_zoom(Zoom { factor: 1.5, smooth: true, turn: 0.8 });
        render::set_minimap(Some(Minimap {
            corner: ShapeCorner::BottomRight,
            size: 64.0,
            blocks_per_pixel: 1.0,
            rotate: false,
            round: true,
        }));
        let label = Shape::Text(TextShape {
            text: "tour".into(),
            at: Point { x: 2.0, y: 2.0 },
            color: 0xFFFFFF,
            centered: false,
            shadow: true,
        });
        render::set_corner(ShapeCorner::TopLeft, 40.0, 12.0, std::slice::from_ref(&label));
        render::set_hud(std::slice::from_ref(&label));
        let visits = storage::get("tour").map_or(0, |v| v.first().copied().unwrap_or(0));
        let _ = storage::set("tour", &[visits.wrapping_add(1)]);
        command::run("/time query daytime");
        command::run_quiet(&["/time query gametime".to_owned()]);
        let player = game::get_player().map_or(0.0, |p| p.y);
        let clock = game::get_clock().is_some();
        let hit = game::raycast(5.0).is_some();
        let held = input::held().map_or(String::new(), |h| h.id);
        let events = input::poll().len();
        input::claim_wheel(false);
        let key = input::mapping("showcase_tour").is_some();
        let (left, right) = input::clicks_per_second();
        let all = recipes::all().len();
        let known = recipes::known();
        if let Some(id) = known.first() {
            let _ = recipes::place(*id, false);
        }
        browser::set_recipes("Showcase", &[], 0, &[]);
        let browsed = browser::poll().len();
        ui::close_screen();
        format!(
            "y {player:.1}, clock {clock}, looking at a block {hit}, holding {held:?}, {events} input events, key {key}, \
             clicks {left}/{right}, {all} recipes ({} known), {browsed} browser events, {} fps, \"tour\" is {} wide",
            known.len(),
            render::frame_stats().fps,
            render::text_width("tour"),
        )
    }
}

impl Mod for ShowcaseHud {
    fn init() -> Result<(Self, Registration), String> {
        reactor_layout(0.0);
        // A page in Options > Mods, and a key: both are the mod's.
        ui::set_settings(&[Widget::Text(TextWidget {
            id: 1,
            text: "Core Showcase has no settings: this page is here because it can be.".into(),
            color: 0,
        })]);
        input::register_key("showcase_tour", "Showcase tour", "key.keyboard.k")?;
        input::claim_items(&["showcase:wand".to_owned()]);
        Ok((
            ShowcaseHud::default(),
            Registration::new()
                .command("showcase_ping")
                .command("showcase_panel")
                .command("showcase_tour")
                .command("showcase_screen")
                .command("showcase_far")
                .command("showcase_inventory"),
        ))
    }

    fn command(&mut self, name: &str, _args: &str) -> Result<Option<String>, String> {
        if name == "showcase_far" {
            self.far = !self.far;
            let far = self.far.then_some(FarTerrain { distance: 64, reduced: true });
            render::set_far_terrain(far);
            return Ok(Some(format!("Far terrain: {}", self.far)));
        }
        if name == "showcase_screen" {
            ui::open_screen(&Screen {
                title: "Core Showcase".into(),
                widgets: vec![Widget::Text(TextWidget {
                    id: 1,
                    text: "A plain screen of widgets; /showcase_panel draws its own.".into(),
                    color: 0,
                })],
            });
            return Ok(None);
        }
        if name == "showcase_tour" {
            return Ok(Some(self.tour()));
        }
        if name == "showcase_inventory" {
            self.replaced = !self.replaced;
            let screen = self.replaced.then(|| Screen {
                title: "Core Showcase".into(),
                widgets: vec![Widget::Text(TextWidget {
                    id: 1,
                    text: "The inventory key shows this now; /showcase_inventory undoes it.".into(),
                    color: 0,
                })],
            });
            ui::replace_screen("minecraft:inventory", screen.as_ref())?;
            return Ok(None);
        }
        if name == "showcase_panel" {
            client_net::send("showcase/slots", b"")?;
            return Ok(None);
        }
        client_net::send("showcase/ping", b"")?;
        Ok(None)
    }

    fn ui_select(&mut self, id: u32, _row: u32) {
        let _ = client_net::send("showcase/take", &[id as u8]);
    }

    fn net_received(&mut self, channel: &str, data: &[u8]) {
        let text = String::from_utf8_lossy(data).into_owned();
        match channel {
            "showcase/status" => hud::set_panel(
                Corner::TopRight,
                &[
                    Line {
                        text: "Core Showcase".into(),
                        color: 0xFFD040,
                    },
                    Line {
                        text,
                        color: 0xFFFFFF,
                    },
                ],
            ),
            "showcase/charge" => reactor_layout(text.parse().unwrap_or(0.0)),
            "showcase/slots" => panel(&text),
            "showcase/rings" => rings(&text),
            "showcase/pong" => chat::show(&format!("Server says: {text}")),
            _ => {}
        }
    }
}


//! In-canvas UI for OpenRA Rust: joystick, zoom, burger, dev-meny, sprakvelger
//! og produksjonsko -- ALT tegnet/handtert i Rust slik at spillet er native-klart
//! (iPhone/Android/desktop) uten JS/HTML-overlay.
//!
//! Metodene henger pa `Game` (definert i main.rs). Som barnemodul har `ui` tilgang
//! til `Game`s private felt/metoder.

use macroquad::prelude::*;

use crate::i18n::{self, Key};
use crate::{
    bridge, levels, team_color, txt, txt_measure, unit_stats, BuildingKind, Game, Unit, UnitKind,
    MAP_H, MAP_W, SIDEBAR_W, TEAM_ENEMY, TEAM_PLAYER, TILE,
};


// En rad i produksjonsko-popupen.
#[derive(Clone, Copy)]
struct QRow {
    kind: UnitKind,
    building: bool, // true = aktiv byggeplass (under bygging), false = i ko
    idx: usize,     // indeks i active hhv. queue
    frac: f32,      // fremdrift 0..1 (kun for aktive)
}

// Dev-handlinger i in-canvas dev-panelet (alt i Rust).
#[derive(Clone, Copy, PartialEq)]
enum DevAct {
    Close,
    Give,
    Inf,
    Tank,
    Harv,
    SpawnYou,
    SpawnFoe,
    Pause,
    Free,
    God,
    Reveal,
    Speed,
    Restart,
    Center,
    Sound,
    Win, // vinn nivaet umiddelbart (testing / hopp)
}

impl Game {
    // ======================================================================
    // In-canvas UI (joystick, zoom, burger, dev-meny, sprakvelger) -- ALT i
    // Rust slik at spillet er native-klart (iPhone/Android/desktop) uten
    // JS/HTML-overlay. Tegnes i draw_controls, handteres i handle_ui.
    // ======================================================================

    // --- Geometri (logiske px; konsistent pa tvers av enheter) ---
    fn ui_joy(&self) -> (Vec2, f32) {
        let r = 56.0;
        let c = vec2(self.play_w() - r - 24.0, screen_height() - r - 24.0);
        (c, r)
    }
    fn ui_zoom_in(&self) -> Rect {
        Rect::new(10.0, 38.0, 38.0, 38.0)
    }
    fn ui_zoom_out(&self) -> Rect {
        Rect::new(10.0, 84.0, 38.0, 38.0)
    }
    fn ui_burger(&self) -> Rect {
        // Under den bla HUD-linja, pa samme hoyde som zoom "+".
        Rect::new(screen_width() - 58.0, 38.0, 52.0, 36.0)
    }
    // Dev og sprakvelger ligger nederst i byggmenyen (burger/sidebar) -- borte
    // fra det mobil-uvennlige nedre venstre hjornet. Kun aktive nar sidebaren er
    // apen (apnes via burger eller ved a velge fabrikken).
    fn ui_dev_btn(&self) -> Rect {
        let x = self.play_w() + 8.0;
        let half = (SIDEBAR_W - 16.0 - 6.0) / 2.0;
        Rect::new(x, screen_height() - 30.0, half, 24.0)
    }
    fn ui_lang_btn(&self) -> Rect {
        let x = self.play_w() + 8.0;
        let half = (SIDEBAR_W - 16.0 - 6.0) / 2.0;
        Rect::new(x + half + 6.0, screen_height() - 30.0, half, 24.0)
    }
    // Seier/tap-panelets bunnboks. Returnerer (Spill igjen, evt. Neste niva).
    // Neste-knapp kun ved seier og hvis det finnes flere nivaer.
    fn outcome_box(&self) -> Rect {
        let bw = 320.0_f32.min(screen_width() - 40.0);
        let bh = 150.0;
        Rect::new((screen_width() - bw) * 0.5, (screen_height() - bh) * 0.5, bw, bh)
    }
    fn outcome_btns(&self) -> (Rect, Option<Rect>) {
        let b = self.outcome_box();
        let y = b.y + b.h - 46.0;
        let has_next = self.outcome == Some(true) && self.level + 1 < levels::count();
        if has_next {
            let w = (b.w - 24.0) * 0.5;
            (
                Rect::new(b.x + 8.0, y, w, 34.0),
                Some(Rect::new(b.x + 8.0 + w + 8.0, y, w, 34.0)),
            )
        } else {
            (Rect::new(b.x + 8.0, y, b.w - 16.0, 34.0), None)
        }
    }
    fn dev_warn_btns(&self) -> (Rect, Rect) {
        let bw = 320.0_f32.min(screen_width() - 40.0);
        let bh = 130.0;
        let bx = (screen_width() - bw) * 0.5;
        let by = (screen_height() - bh) * 0.5;
        let w = (bw - 24.0) * 0.5;
        let y = by + bh - 42.0;
        (
            Rect::new(bx + 8.0, y, w, 30.0),
            Rect::new(bx + 8.0 + w + 8.0, y, w, 30.0),
        )
    }
    fn dev_items(&self) -> Vec<(Rect, DevAct)> {
        let acts = [
            DevAct::Close, DevAct::Give, DevAct::Inf, DevAct::Tank, DevAct::Harv,
            DevAct::SpawnYou, DevAct::SpawnFoe, DevAct::Pause, DevAct::Free, DevAct::God,
            DevAct::Reveal, DevAct::Speed, DevAct::Restart, DevAct::Center, DevAct::Sound,
            DevAct::Win,
        ];
        let (x0, y0) = (12.0, 176.0);
        let (cw, ch, gx, gy) = (112.0, 24.0, 6.0, 5.0);
        let cols = 2;
        acts.iter()
            .enumerate()
            .map(|(i, &a)| {
                let c = (i % cols) as f32;
                let r = (i / cols) as f32;
                (Rect::new(x0 + c * (cw + gx), y0 + r * (ch + gy), cw, ch), a)
            })
            .collect()
    }
    fn dev_panel_rect(&self) -> Rect {
        let items = self.dev_items();
        let last = items.last().map(|(r, _)| r.y + r.h).unwrap_or(176.0);
        Rect::new(6.0, 128.0, 242.0, last - 128.0 + 8.0)
    }
    /// Er punktet over en in-canvas-kontroll? Brukes til a stoppe kant-scroll
    /// og verdens-klikk nar man bruker UI/menyer.
    pub(crate) fn point_in_ui(&self, p: Vec2) -> bool {
        if self.outcome.is_some() {
            return true; // seier/tap-panel er modalt
        }
        let (jc, jr) = self.ui_joy();
        if p.distance(jc) <= jr {
            return true;
        }
        if self.ui_zoom_in().contains(p)
            || self.ui_zoom_out().contains(p)
            || self.ui_burger().contains(p)
        {
            return true;
        }
        if self.sidebar_on() && (self.ui_dev_btn().contains(p) || self.ui_lang_btn().contains(p)) {
            return true;
        }
        if self.dev_warn {
            return true; // modal dekker alt
        }
        if self.dev_open && self.dev_panel_rect().contains(p) {
            return true;
        }
        if self.lang_open && self.lang_panel_rect().contains(p) {
            return true;
        }
        if self.queue_open && !self.sidebar_on() && self.queue_panel_rect().contains(p) {
            return true;
        }
        if self.prod_compact_rect().contains(p) {
            return true;
        }
        false
    }
    fn lang_panel_rect(&self) -> Rect {
        let w = 210.0_f32.min(screen_width() - 20.0);
        let top = 74.0;
        let bottom = screen_height() - 44.0;
        let h = (bottom - top).clamp(120.0, 9.0 + 44.0 * 30.0);
        Rect::new(10.0, top, w, h)
    }
    fn lang_row_h(&self) -> f32 {
        30.0
    }

    // ----- Produksjonsko (popup nar byggmenyen er lukket) -----
    fn queue_rows(&self) -> Vec<QRow> {
        let p = &self.prod[TEAM_PLAYER as usize];
        let mut v = Vec::new();
        for (i, (k, rem)) in p.active.iter().enumerate() {
            let frac = (1.0 - rem / unit_stats(*k).build_time).clamp(0.0, 1.0);
            v.push(QRow { kind: *k, building: true, idx: i, frac });
        }
        for (i, k) in p.queue.iter().enumerate() {
            v.push(QRow { kind: *k, building: false, idx: i, frac: 0.0 });
        }
        v
    }
    // Kompakt produksjons-stripe (oppe til hoyre nar sidebaren er lukket). Tom
    // rekt nar det ikke er noe a vise.
    fn prod_compact_rect(&self) -> Rect {
        let p = &self.prod[TEAM_PLAYER as usize];
        if self.sidebar_on() || (p.active.is_empty() && p.queue.is_empty()) {
            return Rect::new(0.0, 0.0, 0.0, 0.0);
        }
        let w = 156.0;
        let h = p.active.len() as f32 * 22.0 + if p.queue.is_empty() { 0.0 } else { 18.0 } + 6.0;
        Rect::new(screen_width() - w - 8.0, 34.0, w, h.max(20.0))
    }
    fn queue_panel_rect(&self) -> Rect {
        let rows = self.queue_rows().len().max(1) as f32;
        let w = 230.0_f32.min(screen_width() - 16.0);
        let h = 30.0 + rows * 26.0 + 6.0;
        Rect::new(screen_width() - w - 8.0, 34.0, w, h)
    }
    fn queue_row_rect(&self, i: usize) -> Rect {
        let p = self.queue_panel_rect();
        Rect::new(p.x + 4.0, p.y + 30.0 + i as f32 * 26.0, p.w - 8.0, 24.0)
    }
    fn queue_cancel(&mut self, row: QRow) {
        let p = &mut self.prod[TEAM_PLAYER as usize];
        if row.building {
            if row.idx < p.active.len() {
                p.active.remove(row.idx);
            }
        } else if row.idx < p.queue.len() {
            p.queue.remove(row.idx);
        }
        // Refunder kostnaden (full -- betalt ved bestilling).
        self.credits[TEAM_PLAYER as usize] += unit_stats(row.kind).cost;
    }
    fn queue_move(&mut self, qidx: usize, up: bool) {
        let p = &mut self.prod[TEAM_PLAYER as usize];
        if up {
            if qidx > 0 && qidx < p.queue.len() {
                p.queue.swap(qidx, qidx - 1);
            }
        } else if qidx + 1 < p.queue.len() {
            p.queue.swap(qidx, qidx + 1);
        }
    }
    fn lang_max_scroll(&self) -> f32 {
        let panel = self.lang_panel_rect();
        let content = i18n::LANGS.len() as f32 * self.lang_row_h() + 8.0;
        (content - panel.h).max(0.0)
    }

    fn ui_zoom(&mut self, factor: f32) {
        let center = vec2(self.play_w() * 0.5, screen_height() * 0.5);
        let before = self.screen_to_world(center);
        self.zoom = (self.zoom * factor).clamp(0.4, 3.0);
        let after = self.screen_to_world(center);
        self.cam += before - after;
        self.clamp_camera();
    }

    fn dev_label(&self, a: DevAct) -> String {
        let on = |b: bool| if b { self.t(Key::DevOn) } else { self.t(Key::DevOff) };
        match a {
            DevAct::Close => self.t(Key::DevClose).to_string(),
            DevAct::Give => self.t(Key::DevGive).to_string(),
            DevAct::Inf => self.t(Key::DevBuildInf).to_string(),
            DevAct::Tank => self.t(Key::DevBuildTank).to_string(),
            DevAct::Harv => self.t(Key::DevBuildHarv).to_string(),
            DevAct::SpawnYou => self.t(Key::DevSpawnTankYou).to_string(),
            DevAct::SpawnFoe => self.t(Key::DevSpawnTankFoe).to_string(),
            DevAct::Pause => format!("{} {}", self.t(Key::DevPause), on(self.paused)),
            DevAct::Free => format!("{} {}", self.t(Key::DevFreeBuild), on(self.free_build)),
            DevAct::God => format!("{} {}", self.t(Key::DevGod), on(self.god_mode)),
            DevAct::Reveal => format!("{} {}", self.t(Key::DevReveal), on(self.reveal)),
            DevAct::Speed => format!("{} x{}", self.t(Key::DevSpeed), self.speed as i32),
            DevAct::Restart => self.t(Key::DevRestart).to_string(),
            DevAct::Center => self.t(Key::DevCenter).to_string(),
            DevAct::Sound => format!("{} {}", self.t(Key::DevSound), on(!self.muted)),
            DevAct::Win => self.t(Key::DevWin).to_string(),
        }
    }

    fn center_on_base(&mut self) {
        let focus = self
            .buildings
            .iter()
            .find(|b| b.team == TEAM_PLAYER && b.kind == BuildingKind::Hq)
            .map(|b| b.pos)
            .or_else(|| self.units.iter().find(|u| u.team == TEAM_PLAYER).map(|u| u.pos))
            .unwrap_or_else(|| vec2(MAP_W as f32 * TILE, MAP_H as f32 * TILE) * 0.5);
        let view = vec2(self.play_w(), screen_height()) / self.zoom;
        self.cam = focus - view * 0.5;
        self.clamp_camera();
    }

    fn dev_apply(&mut self, a: DevAct) {
        let world_center = self.screen_to_world(vec2(self.play_w() * 0.5, screen_height() * 0.5));
        match a {
            DevAct::Close => self.dev_open = false,
            DevAct::Give => self.credits[TEAM_PLAYER as usize] += 5000.0,
            DevAct::Inf => self.build(TEAM_PLAYER, UnitKind::Rifleman),
            DevAct::Tank => self.build(TEAM_PLAYER, UnitKind::Tank),
            DevAct::Harv => self.build(TEAM_PLAYER, UnitKind::Harvester),
            DevAct::SpawnYou => {
                self.units.push(Unit::new(world_center, TEAM_PLAYER, UnitKind::Tank));
                self.outcome = None;
            }
            DevAct::SpawnFoe => {
                self.units.push(Unit::new(world_center, TEAM_ENEMY, UnitKind::Tank));
                self.outcome = None;
            }
            DevAct::Pause => self.paused = !self.paused,
            DevAct::Free => self.free_build = !self.free_build,
            DevAct::God => self.god_mode = !self.god_mode,
            DevAct::Reveal => self.reveal = !self.reveal,
            DevAct::Speed => {
                self.speed = match self.speed as i32 {
                    1 => 2.0,
                    2 => 4.0,
                    _ => 1.0,
                };
            }
            DevAct::Restart => {
                let lvl = self.level;
                self.load_level(lvl); // spill gjeldende niva pa nytt
                self.dev_open = true;
                self.ui_init = true;
            }
            DevAct::Center => self.center_on_base(),
            DevAct::Sound => {
                self.muted = !self.muted;
                bridge::set_muted(self.muted);
            }
            DevAct::Win => self.outcome = Some(true),
        }
    }

    /// Handter all in-canvas UI. Settes `ui_block` nar pekeren brukes av UI sa
    /// verdens-seleksjon (handle_selection) ikke ogsa reagerer.
    pub(crate) fn handle_ui(&mut self) {
        self.ui_block = false;
        // Forste frame: byggmeny apen pa romslige skjermer, lukket pa mobil.
        if !self.ui_init {
            self.sidebar_open = screen_width() > 560.0;
            self.ui_init = true;
        }

        let (mx, my) = mouse_position();
        let m = vec2(mx, my);
        let pressed = is_mouse_button_pressed(MouseButton::Left);
        let down = is_mouse_button_down(MouseButton::Left);
        let released = is_mouse_button_released(MouseButton::Left);
        if pressed {
            self.ui_press = m;
        }

        // --- Seier/tap-panel (modalt) ---
        if self.outcome.is_some() {
            self.ui_block = true;
            if pressed {
                let win = self.outcome == Some(true);
                let (replay, next) = self.outcome_btns();
                if replay.contains(m) {
                    let l = self.level;
                    self.load_level(l);
                    return;
                }
                if win {
                    if let Some(nx) = next {
                        if nx.contains(m) {
                            let l = self.level + 1;
                            self.load_level(l);
                            return;
                        }
                    }
                }
            }
            return;
        }

        // --- Joystick (eier pekeren mens den dras) ---
        let (jc, jr) = self.ui_joy();
        if self.joy_active {
            if down {
                let d = m - jc;
                let v = if d.length() > jr { d.normalize() } else { d / jr };
                self.joy_vec = v;
                self.pan_vel = v;
                self.ui_block = true;
            } else {
                self.joy_active = false;
                self.joy_vec = Vec2::ZERO;
                self.pan_vel = Vec2::ZERO;
            }
            return;
        }
        if pressed && m.distance(jc) <= jr * 1.25 {
            self.joy_active = true;
            let d = m - jc;
            let v = if d.length() > jr { d.normalize() } else { d / jr };
            self.joy_vec = v;
            self.pan_vel = v;
            self.ui_block = true;
            return;
        }

        // --- Modal: dev-advarsel (sluker klikk under) ---
        if self.dev_warn {
            self.ui_block = true;
            if pressed {
                let (acc, can) = self.dev_warn_btns();
                if acc.contains(m) {
                    self.cheater = true;
                    self.dev_warn = false;
                    self.dev_open = true;
                } else if can.contains(m) {
                    self.dev_warn = false;
                }
            }
            return;
        }

        // --- Sprakvelger (rullbar liste) ---
        if self.lang_open {
            let panel = self.lang_panel_rect();
            let (_, wy) = mouse_wheel();
            if wy != 0.0 && panel.contains(m) {
                self.lang_scroll = (self.lang_scroll - wy * 30.0).clamp(0.0, self.lang_max_scroll());
            }
            if pressed && panel.contains(m) {
                self.lang_dragging = false;
                self.ui_block = true;
            } else if pressed {
                // Trykk utenfor panelet lukker lista.
                self.lang_open = false;
                self.ui_block = true;
                return;
            }
            if down && panel.contains(m) {
                let dy = my - self.last_mouse.y;
                if (m - self.ui_press).length() > 6.0 {
                    self.lang_dragging = true;
                }
                if self.lang_dragging {
                    self.lang_scroll = (self.lang_scroll - dy).clamp(0.0, self.lang_max_scroll());
                }
                self.ui_block = true;
            }
            if released && panel.contains(m) && !self.lang_dragging {
                // Tapp pa en rad -> velg sprak.
                let rel = m.y - (panel.y + 4.0) + self.lang_scroll;
                let idx = (rel / self.lang_row_h()).floor() as i32;
                if idx >= 0 && (idx as usize) < i18n::LANGS.len() {
                    self.lang = i18n::from_index(idx as usize);
                    self.lang_open = false;
                }
                self.ui_block = true;
            }
            return;
        }

        // --- Produksjonsko-popup (nar byggmenyen er lukket) ---
        if self.queue_open && !self.sidebar_on() {
            let rows = self.queue_rows();
            if rows.is_empty() {
                self.queue_open = false;
                return;
            }
            let panel = self.queue_panel_rect();
            if pressed && !panel.contains(m) {
                self.queue_open = false;
                self.ui_block = true;
                return;
            }
            if pressed {
                self.ui_block = true;
                for (i, row) in rows.iter().enumerate() {
                    let rr = self.queue_row_rect(i);
                    let xr = Rect::new(rr.x + rr.w - 24.0, rr.y + 2.0, 20.0, 20.0);
                    if xr.contains(m) {
                        self.queue_cancel(*row);
                        return;
                    }
                    if !row.building {
                        let dn = Rect::new(rr.x + rr.w - 48.0, rr.y + 2.0, 20.0, 20.0);
                        let up = Rect::new(rr.x + rr.w - 72.0, rr.y + 2.0, 20.0, 20.0);
                        if up.contains(m) {
                            self.queue_move(row.idx, true);
                            return;
                        }
                        if dn.contains(m) {
                            self.queue_move(row.idx, false);
                            return;
                        }
                    }
                }
            }
            return;
        }

        // --- Verktoylinje (kun pa trykk) ---
        if pressed {
            // Trykk pa kompakt produksjons-stripe -> apne ko-popup.
            let pc = self.prod_compact_rect();
            if pc.w > 0.0 && pc.contains(m) {
                self.queue_open = true;
                self.ui_block = true;
                return;
            }
            if self.ui_zoom_in().contains(m) {
                self.ui_zoom(1.25);
                self.ui_block = true;
                return;
            }
            if self.ui_zoom_out().contains(m) {
                self.ui_zoom(1.0 / 1.25);
                self.ui_block = true;
                return;
            }
            if self.ui_burger().contains(m) {
                self.sidebar_open = !self.sidebar_open;
                self.ui_block = true;
                return;
            }
            // Dev/sprak ligger i sidebaren -> kun nar den er apen.
            if self.sidebar_on() && self.ui_dev_btn().contains(m) {
                if self.cheater {
                    self.dev_open = !self.dev_open;
                } else {
                    self.dev_warn = true;
                }
                self.ui_block = true;
                return;
            }
            if self.sidebar_on() && self.ui_lang_btn().contains(m) {
                self.lang_open = true;
                self.lang_scroll = 0.0;
                self.ui_block = true;
                return;
            }
            if self.dev_open {
                for (rect, a) in self.dev_items() {
                    if rect.contains(m) {
                        self.dev_apply(a);
                        self.ui_block = true;
                        return;
                    }
                }
                // Trykk utenfor panelet (og ikke pa Dev-knappen) lukker det.
                if !self.dev_panel_rect().contains(m) {
                    self.dev_open = false;
                    self.ui_block = true;
                    return;
                }
            }
        }
    }

    // --- Tegning av alle in-canvas-kontroller ---
    pub(crate) fn draw_controls(&self) {
        let (mx, my) = mouse_position();
        let m = vec2(mx, my);
        let accent = team_color(TEAM_PLAYER);
        let panel_bg = Color::new(0.08, 0.11, 0.09, 0.92);
        let btn = |r: Rect, hot: bool, label: &str, fsz: f32, active: bool| {
            let bg = if active {
                Color::new(0.16, 0.30, 0.20, 0.95)
            } else if hot {
                Color::new(0.18, 0.24, 0.22, 0.95)
            } else {
                Color::new(0.10, 0.14, 0.12, 0.92)
            };
            draw_rectangle(r.x, r.y, r.w, r.h, bg);
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5, Color::new(0.25, 0.55, 0.35, 0.9));
            let d = txt_measure(label, fsz);
            txt(label, r.x + (r.w - d.width) * 0.5, r.y + r.h * 0.5 + d.height * 0.35, fsz, Color::new(0.82, 0.95, 0.85, 1.0));
        };

        // Zoom +/-
        let zi = self.ui_zoom_in();
        let zo = self.ui_zoom_out();
        btn(zi, zi.contains(m), "+", 26.0, false);
        btn(zo, zo.contains(m), "-", 26.0, false);

        // Burger (X nar apen)
        let bu = self.ui_burger();
        btn(bu, bu.contains(m), if self.sidebar_open { "X" } else { "=" }, 18.0, false);

        // Dev- og sprakknapp nederst i byggmenyen (kun nar sidebaren er apen).
        if self.sidebar_on() {
            let strip_y = screen_height() - 36.0;
            draw_rectangle(self.play_w(), strip_y, SIDEBAR_W, 36.0, Color::new(0.08, 0.09, 0.10, 1.0));
            let dv = self.ui_dev_btn();
            btn(dv, dv.contains(m), "Dev", 15.0, self.dev_open);
            let lg = self.ui_lang_btn();
            let iso = i18n::LANGS[i18n::index_of(self.lang)].1.to_uppercase();
            btn(lg, lg.contains(m), &iso, 15.0, self.lang_open);
        }

        // Joystick
        let (jc, jr) = self.ui_joy();
        draw_circle(jc.x, jc.y, jr, Color::new(0.10, 0.14, 0.12, 0.55));
        draw_circle_lines(jc.x, jc.y, jr, 2.0, Color::new(0.30, 0.55, 0.40, 0.8));
        let knob = jc + self.joy_vec * jr;
        draw_circle(knob.x, knob.y, jr * 0.42, Color::new(0.30, 0.65, 0.42, 0.95));
        draw_circle_lines(knob.x, knob.y, jr * 0.42, 1.5, Color::new(0.6, 0.9, 0.7, 0.9));

        // Cheater-merke
        if self.cheater {
            let s = self.t(Key::Cheater);
            let d = txt_measure(s, 16.0);
            txt(s, self.play_w() - d.width - 12.0, 46.0, 16.0, Color::new(0.95, 0.45, 0.40, 1.0));
        }

        // Dev-panel
        if self.dev_open {
            let items = self.dev_items();
            let bg = self.dev_panel_rect();
            draw_rectangle(bg.x, bg.y, bg.w, bg.h, panel_bg);
            draw_rectangle_lines(bg.x, bg.y, bg.w, bg.h, 1.5, accent);
            txt(self.t(Key::DevTitle), bg.x + 8.0, bg.y + 18.0, 16.0, Color::new(0.85, 0.95, 1.0, 1.0));
            let econ = format!(
                "{} {}   {} {}",
                self.t(Key::DevCredits),
                self.credits[0] as i32,
                self.t(Key::DevFps),
                get_fps(),
            );
            txt(&econ, bg.x + 8.0, bg.y + 36.0, 13.0, Color::new(0.7, 0.85, 0.75, 1.0));
            for (r, a) in items {
                let label = self.dev_label(a);
                let active = (a == DevAct::Pause && self.paused)
                    || (a == DevAct::Free && self.free_build)
                    || (a == DevAct::God && self.god_mode)
                    || (a == DevAct::Reveal && self.reveal)
                    || (a == DevAct::Sound && !self.muted);
                btn(r, r.contains(m), &label, 12.0, active);
            }
        }

        // Sprakvelger-liste
        if self.lang_open {
            let p = self.lang_panel_rect();
            draw_rectangle(p.x, p.y, p.w, p.h, panel_bg);
            draw_rectangle_lines(p.x, p.y, p.w, p.h, 1.5, accent);
            let rh = self.lang_row_h();
            let cur = i18n::index_of(self.lang);
            for (i, row) in i18n::LANGS.iter().enumerate() {
                let ry = p.y + 4.0 + i as f32 * rh - self.lang_scroll;
                if ry + rh < p.y || ry > p.y + p.h {
                    continue; // utenfor synlig omrade
                }
                let hot = m.x >= p.x && m.x <= p.x + p.w && m.y >= ry && m.y < ry + rh;
                if i == cur {
                    draw_rectangle(p.x + 2.0, ry, p.w - 4.0, rh, Color::new(0.16, 0.30, 0.20, 0.9));
                } else if hot {
                    draw_rectangle(p.x + 2.0, ry, p.w - 4.0, rh, Color::new(0.14, 0.20, 0.16, 0.9));
                }
                let label = format!("{}  ·  {}", row.4, row.3); // engelsk · innfodt
                txt(&label, p.x + 8.0, ry + rh * 0.5 + 5.0, 14.0, Color::new(0.85, 0.95, 0.88, 1.0));
            }
            // klipp-kant sa rader ikke flyter over rammen
            draw_rectangle_lines(p.x, p.y, p.w, p.h, 2.0, accent);
        }

        // Produksjonsko-popup
        if self.queue_open && !self.sidebar_on() {
            let rows = self.queue_rows();
            if rows.is_empty() {
                // (lukkes i handle_ui neste frame)
            } else {
                let panel = self.queue_panel_rect();
                draw_rectangle(panel.x, panel.y, panel.w, panel.h, panel_bg);
                draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 1.5, accent);
                txt(self.t(Key::DevProduction), panel.x + 8.0, panel.y + 20.0, 15.0, Color::new(0.85, 0.95, 1.0, 1.0));
                for (i, row) in rows.iter().enumerate() {
                    let rr = self.queue_row_rect(i);
                    // rad-bakgrunn + fremdrift for aktive
                    draw_rectangle(rr.x, rr.y, rr.w, rr.h, Color::new(0.10, 0.13, 0.12, 0.9));
                    if row.building {
                        draw_rectangle(rr.x, rr.y, rr.w * row.frac, rr.h, Color::new(0.20, 0.45, 0.28, 0.9));
                    }
                    txt(self.t(row.kind.name_key()), rr.x + 6.0, rr.y + 17.0, 14.0, WHITE);
                    if row.building {
                        let pct = format!("{}%", (row.frac * 100.0) as i32);
                        txt(&pct, rr.x + rr.w - 110.0, rr.y + 17.0, 12.0, Color::new(0.7, 0.95, 0.75, 1.0));
                    }
                    // knapper til hoyre: [opp][ned][X] (opp/ned kun for ko-rader)
                    let xr = Rect::new(rr.x + rr.w - 24.0, rr.y + 2.0, 20.0, 20.0);
                    btn(xr, xr.contains(m), "X", 13.0, false);
                    if !row.building {
                        let dn = Rect::new(rr.x + rr.w - 48.0, rr.y + 2.0, 20.0, 20.0);
                        let up = Rect::new(rr.x + rr.w - 72.0, rr.y + 2.0, 20.0, 20.0);
                        btn(up, up.contains(m), "^", 13.0, false);
                        btn(dn, dn.contains(m), "v", 13.0, false);
                    }
                }
            }
        }

        // Dev-advarsel (modal)
        if self.dev_warn {
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.5));
            let bw = 320.0_f32.min(screen_width() - 40.0);
            let bh = 130.0;
            let bx = (screen_width() - bw) * 0.5;
            let by = (screen_height() - bh) * 0.5;
            draw_rectangle(bx, by, bw, bh, Color::new(0.10, 0.12, 0.14, 0.98));
            draw_rectangle_lines(bx, by, bw, bh, 2.0, Color::new(0.9, 0.5, 0.3, 1.0));
            let warn = self.t(Key::DevWarn);
            let d = txt_measure(warn, 17.0);
            txt(warn, bx + (bw - d.width).max(8.0) * 0.5, by + 44.0, 17.0, Color::new(1.0, 0.85, 0.5, 1.0));
            let (acc, can) = self.dev_warn_btns();
            btn(acc, acc.contains(m), self.t(Key::DevAccept), 15.0, false);
            btn(can, can.contains(m), self.t(Key::DevCancel), 15.0, false);
        }

        // Seier/tap-panel (modalt) -- "Spill igjen" / "Neste niva".
        if let Some(win) = self.outcome {
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.55));
            let b = self.outcome_box();
            draw_rectangle(b.x, b.y, b.w, b.h, Color::new(0.10, 0.13, 0.12, 0.98));
            let border = if win { Color::new(0.30, 0.80, 0.42, 1.0) } else { Color::new(0.90, 0.40, 0.30, 1.0) };
            draw_rectangle_lines(b.x, b.y, b.w, b.h, 2.0, border);
            let title = if win {
                if self.level + 1 >= levels::count() {
                    self.t(Key::CampaignDone).to_string()
                } else {
                    format!("{} {} {}", self.t(Key::Level), self.level + 1, self.t(Key::LevelComplete))
                }
            } else {
                self.t(Key::Defeat).to_string()
            };
            let tcol = if win { Color::new(0.70, 1.0, 0.75, 1.0) } else { Color::new(1.0, 0.6, 0.5, 1.0) };
            let d = txt_measure(&title, 26.0);
            txt(&title, b.x + (b.w - d.width).max(8.0) * 0.5, b.y + 50.0, 26.0, tcol);
            let (replay, next) = self.outcome_btns();
            btn(replay, replay.contains(m), self.t(Key::Replay), 16.0, false);
            if let Some(nx) = next {
                btn(nx, nx.contains(m), self.t(Key::NextLevel), 16.0, false);
            }
        }
    }
}

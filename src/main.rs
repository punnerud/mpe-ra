//! OpenRA Rust — en liten sanntidsstrategi (RTS) i OpenRA-and.
//!
//! Kjorer bade nativt og i nettleser (WASM + WebGL2) via macroquad.
//!
//! Spillmekanikk:
//!   - Okonomi: hostere samler malm og leverer pa raffineri -> kreditter
//!   - Bygninger: HK (mister du -> tap), raffineri, fabrikk
//!   - Produksjon: bygg infanteri / stridsvogn / hoster fra fabrikken
//!   - Kamp: enheter skyter fiender og bygninger innen rekkevidde
//!   - Pathfinding: A* ruter enheter rundt bygninger og ufremkommelig terreng
//!   - Krigstaake (fog of war): uutforsket / utforsket / synlig
//!
//! Styring:
//!   - Venstre museknapp + dra  : boks-selekter egne enheter
//!   - Venstreklikk             : velg enhet eller bygning
//!   - Hoyreklikk               : flytt valgte enheter (eller sett samlepunkt
//!                                 hvis en fabrikk er valgt)
//!   - 1 / 2 / 3                : bygg infanteri / stridsvogn / hoster
//!   - WASD / piltaster         : panorer kamera
//!   - Musehjul                 : zoom
//!   - R                        : start pa nytt

use macroquad::prelude::*;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

mod i18n;
mod levels;
mod ui;
use i18n::{Key, Lang};

// ---------------------------------------------------------------------------
// Font + tekst-hjelpere
// ---------------------------------------------------------------------------
// All tekst tegnes med en Unicode-font (Arial Unicode) sa æøå, gresk, kyrillisk,
// CJK, arabisk osv. vises riktig. Fonten lastes en gang ved oppstart og lagres
// trad-lokalt slik at alle tegne-funksjoner nar den uten a tre den gjennom.
thread_local! {
    static FONT: std::cell::RefCell<Option<Font>> = const { std::cell::RefCell::new(None) };
}

fn set_font(f: Font) {
    FONT.with(|c| *c.borrow_mut() = Some(f));
}

/// Tegn tekst med den lastede Unicode-fonten (fallback til standardfont).
fn txt(s: &str, x: f32, y: f32, size: f32, color: Color) {
    FONT.with(|c| {
        let fb = c.borrow();
        draw_text_ex(
            s,
            x,
            y,
            TextParams {
                font: fb.as_ref(),
                font_size: size.max(1.0) as u16,
                color,
                ..Default::default()
            },
        );
    });
}

/// Mal tekst med samme font som `txt`.
fn txt_measure(s: &str, size: f32) -> TextDimensions {
    FONT.with(|c| {
        let fb = c.borrow();
        measure_text(s, fb.as_ref(), size.max(1.0) as u16, 1.0)
    })
}

// ---------------------------------------------------------------------------
// JS <-> WASM-bro
// ---------------------------------------------------------------------------
mod bridge {
    #[cfg(target_arch = "wasm32")]
    extern "C" {
        fn js_report(
            cam_x: f32,
            cam_y: f32,
            zoom: f32,
            mouse_x: f32,
            mouse_y: f32,
            mouse_active: i32,
            players: i32,
            enemies: i32,
            selected: i32,
            fps: i32,
            outcome: i32,
        );
        fn js_report_econ(
            credits_p: i32,
            credits_e: i32,
            bld_p: i32,
            bld_e: i32,
            queue_len: i32,
            queue_pct: i32,
            speed: f32,
            flags: i32,
        );
        fn js_report_fog(explored_pct: i32, visible_tiles: i32, reveal: i32);
        fn js_poll_cmd() -> i32;
        fn js_poll_arg(i: i32) -> f32;
        fn js_sound(id: i32);
    }

    use std::cell::Cell;
    thread_local! {
        // Lyd av/pa -- styres av dev-menyen (Rust). Synthen ligger fortsatt i JS
        // i denne fasen; demping gjores her sa ingen lyd sendes over broen.
        static MUTED: Cell<bool> = const { Cell::new(false) };
    }
    pub fn set_muted(m: bool) {
        MUTED.with(|c| c.set(m));
    }
    pub fn is_muted() -> bool {
        MUTED.with(|c| c.get())
    }

    // Spill en lyd-effekt (syntetiseres i JS via Web Audio). No-op nativt.
    pub fn sound(id: i32) {
        if is_muted() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        unsafe {
            js_sound(id);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = id;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn report(
        cam_x: f32,
        cam_y: f32,
        zoom: f32,
        mouse_x: f32,
        mouse_y: f32,
        mouse_active: bool,
        players: i32,
        enemies: i32,
        selected: i32,
        fps: i32,
        outcome: i32,
    ) {
        #[cfg(target_arch = "wasm32")]
        unsafe {
            js_report(
                cam_x, cam_y, zoom, mouse_x, mouse_y, mouse_active as i32, players, enemies,
                selected, fps, outcome,
            );
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (
            cam_x, cam_y, zoom, mouse_x, mouse_y, mouse_active, players, enemies, selected, fps,
            outcome,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn report_econ(
        credits_p: i32,
        credits_e: i32,
        bld_p: i32,
        bld_e: i32,
        queue_len: i32,
        queue_pct: i32,
        speed: f32,
        flags: i32,
    ) {
        #[cfg(target_arch = "wasm32")]
        unsafe {
            js_report_econ(credits_p, credits_e, bld_p, bld_e, queue_len, queue_pct, speed, flags);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (credits_p, credits_e, bld_p, bld_e, queue_len, queue_pct, speed, flags);
    }

    pub fn report_fog(explored_pct: i32, visible_tiles: i32, reveal: bool) {
        #[cfg(target_arch = "wasm32")]
        unsafe {
            js_report_fog(explored_pct, visible_tiles, reveal as i32);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (explored_pct, visible_tiles, reveal);
    }

    pub fn poll() -> (i32, [f32; 4]) {
        #[cfg(target_arch = "wasm32")]
        unsafe {
            let args = [js_poll_arg(0), js_poll_arg(1), js_poll_arg(2), js_poll_arg(3)];
            let code = js_poll_cmd();
            (code, args)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            (0, [0.0; 4])
        }
    }
}

// ---------------------------------------------------------------------------
// Konstanter
// ---------------------------------------------------------------------------

const TILE: f32 = 32.0;
const MAP_W: usize = 64;
const MAP_H: usize = 48;

// Bredde pa hoyre sidebar (byggmeny + minikart), C&C-stil.
const SIDEBAR_W: f32 = 150.0;

const TEAM_PLAYER: u8 = 0;
const TEAM_ENEMY: u8 = 1;

const SHOT_LIFETIME: f32 = 0.08;

const HARVEST_CAPACITY: f32 = 300.0;
const HARVEST_RATE: f32 = 90.0;
const ORE_PER_TILE: f32 = 240.0;
const UNLOAD_TIME: f32 = 2.5; // lossetid pa raffineriet (en host om gangen = flaskehals)

// Lyd-effekter (syntetiseres i JS). Aggregeres pr. frame for a unnga spam.
const SND_SHOOT: i32 = 1;
const SND_EXPLOSION: i32 = 2;
const SND_PLACE: i32 = 3;
const SND_READY: i32 = 4;
const SND_UNLOAD: i32 = 5;
const SND_TURRET: i32 = 6;
const SND_WIN: i32 = 7;
const SND_LOSE: i32 = 8;

// Fiende-AI. (Bolge-/tick-tall er na per niva i Game-feltene, satt fra LevelSpec.)
const ENEMY_DESIRED_HARVESTERS: usize = 2; // bygg opp okonomi for haer
const ENEMY_DEFEND_RADIUS: f32 = 360.0; // trusler innenfor denne fra HK = forsvar

// Lagfarge (faction-trim): spiller bla, fiende rod -- C&C/Red Alert-stil.
// Fiendens fargetint settes av gjeldende nivas stil (Balanced/Armor/Swarm), sa
// "to ulike fiender" foles distinkt. Lagres globalt sa team_color() beholder
// signaturen og tinter alt (enheter, bygg, minikart).
thread_local! {
    static ENEMY_TINT: std::cell::Cell<(f32, f32, f32)> =
        const { std::cell::Cell::new((0.90, 0.30, 0.25)) };
}
fn set_enemy_tint(style: levels::EnemyStyle) {
    let c = match style {
        levels::EnemyStyle::Balanced => (0.90, 0.30, 0.25), // rod
        levels::EnemyStyle::Armor => (0.85, 0.62, 0.20),    // amber
        levels::EnemyStyle::Swarm => (0.72, 0.34, 0.85),    // fiolett
    };
    ENEMY_TINT.with(|t| t.set(c));
}
fn team_color(team: u8) -> Color {
    if team == TEAM_PLAYER {
        Color::new(0.28, 0.55, 0.95, 1.0)
    } else {
        let (r, g, b) = ENEMY_TINT.with(|t| t.get());
        Color::new(r, g, b, 1.0)
    }
}

// ---------------------------------------------------------------------------
// Terreng
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Terrain {
    Grass,
    Ore,
    Water,
    Rock,
}

impl Terrain {
    fn color(self, x: usize, y: usize) -> Color {
        let dark = (x + y) % 2 == 0;
        match self {
            Terrain::Grass => {
                if dark {
                    Color::new(0.22, 0.40, 0.16, 1.0)
                } else {
                    Color::new(0.26, 0.45, 0.19, 1.0)
                }
            }
            Terrain::Ore => {
                if dark {
                    Color::new(0.62, 0.52, 0.18, 1.0)
                } else {
                    Color::new(0.70, 0.58, 0.22, 1.0)
                }
            }
            Terrain::Water => {
                if dark {
                    Color::new(0.13, 0.27, 0.45, 1.0)
                } else {
                    Color::new(0.16, 0.31, 0.50, 1.0)
                }
            }
            Terrain::Rock => {
                if dark {
                    Color::new(0.30, 0.30, 0.32, 1.0)
                } else {
                    Color::new(0.36, 0.36, 0.38, 1.0)
                }
            }
        }
    }

    fn passable(self) -> bool {
        !matches!(self, Terrain::Water | Terrain::Rock)
    }
}

fn hash2(x: i32, y: i32) -> f32 {
    let mut h = (x.wrapping_mul(374_761_393)).wrapping_add(y.wrapping_mul(668_265_263));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h as u32 as f32) / (u32::MAX as f32)
}

fn carve_blob(map: &mut [Terrain], cx: i32, cy: i32, r: f32, t: Terrain) {
    let ri = r.ceil() as i32 + 1;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let x = cx + dx;
            let y = cy + dy;
            if x < 0 || y < 0 || x as usize >= MAP_W || y as usize >= MAP_H {
                continue;
            }
            let wobble = hash2(x, y) * 1.5;
            if ((dx * dx + dy * dy) as f32).sqrt() <= r + wobble - 0.7 {
                map[y as usize * MAP_W + x as usize] = t;
            }
        }
    }
}

// Plasser en standardbase (HQ + raffineri + fabrikk + 1 hoster + 3 infanteri)
// rundt `base`. Spilleren speiler bygg-utlegget mot enheten. `power` skalerer
// fiende-enheters hp (vanskelighet). Returnerer samlepunktet.
fn spawn_base(units: &mut Vec<Unit>, buildings: &mut Vec<Building>, base: Vec2, team: u8, power: f32) -> Vec2 {
    let m = if team == TEAM_PLAYER { -1.0 } else { 1.0 };
    buildings.push(Building::new(base, team, BuildingKind::Hq));
    buildings.push(Building::new(base + vec2(150.0 * m, 20.0), team, BuildingKind::Refinery));
    buildings.push(Building::new(base + vec2(40.0 * m, 150.0), team, BuildingKind::Factory));
    let mut harv = Unit::new(base + vec2(150.0 * m, 120.0), team, UnitKind::Harvester);
    let mut rifles: Vec<Unit> = (0..3)
        .map(|i| Unit::new(base + vec2(60.0 + i as f32 * 30.0, 60.0), team, UnitKind::Rifleman))
        .collect();
    if team == TEAM_ENEMY && power != 1.0 {
        harv.hp *= power;
        harv.max_hp *= power;
        for u in &mut rifles {
            u.hp *= power;
            u.max_hp *= power;
        }
    }
    units.push(harv);
    units.extend(rifles);
    base + vec2(40.0 * m, 230.0)
}

// ---------------------------------------------------------------------------
// Pathfinding (A* pa rutenett, 8 retninger)
// ---------------------------------------------------------------------------

#[inline]
fn in_bounds(x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && (x as usize) < MAP_W && (y as usize) < MAP_H
}

#[inline]
fn tile_center(x: i32, y: i32) -> Vec2 {
    vec2((x as f32 + 0.5) * TILE, (y as f32 + 0.5) * TILE)
}

/// Naermeste apne rute til (gx,gy) via ringsok (brukes nar malet er blokkert,
/// f.eks. en bygning).
fn nearest_free(blocked: &[bool], gx: i32, gy: i32) -> Option<(i32, i32)> {
    for r in 0..12i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue; // kun ringen
                }
                let x = gx + dx;
                let y = gy + dy;
                if in_bounds(x, y) && !blocked[y as usize * MAP_W + x as usize] {
                    return Some((x, y));
                }
            }
        }
    }
    None
}

/// A* fra `start` til `goal` (verdenskoordinater). Returnerer waypoints (rute-
/// sentre), uten startruta. Tom hvis allerede framme eller ingen sti finnes.
fn astar(blocked: &[bool], start: Vec2, goal: Vec2) -> Vec<Vec2> {
    let (sx, sy) = ((start.x / TILE).floor() as i32, (start.y / TILE).floor() as i32);
    let (mut gx, mut gy) = ((goal.x / TILE).floor() as i32, (goal.y / TILE).floor() as i32);
    if !in_bounds(sx, sy) {
        return vec![];
    }
    let gx0 = gx.clamp(0, MAP_W as i32 - 1);
    let gy0 = gy.clamp(0, MAP_H as i32 - 1);
    gx = gx0;
    gy = gy0;
    if (sx, sy) == (gx, gy) {
        return vec![];
    }
    if blocked[gy as usize * MAP_W + gx as usize] {
        match nearest_free(blocked, gx, gy) {
            Some((nx, ny)) => {
                gx = nx;
                gy = ny;
            }
            None => return vec![],
        }
    }
    if (sx, sy) == (gx, gy) {
        return vec![];
    }

    let n = MAP_W * MAP_H;
    let idx = |x: i32, y: i32| y as usize * MAP_W + x as usize;
    let start_i = idx(sx, sy);
    let goal_i = idx(gx, gy);

    let mut g_score = vec![i32::MAX; n];
    let mut came = vec![usize::MAX; n];
    let mut closed = vec![false; n];
    g_score[start_i] = 0;

    let h = |x: i32, y: i32| -> i32 {
        let dx = (x - gx).abs();
        let dy = (y - gy).abs();
        100 * dx.max(dy) + 41 * dx.min(dy)
    };

    let mut open: BinaryHeap<Reverse<(i32, usize)>> = BinaryHeap::new();
    open.push(Reverse((h(sx, sy), start_i)));

    // 8 naboer: (dx, dy, kostnad, diagonal)
    let dirs: [(i32, i32, i32, bool); 8] = [
        (1, 0, 100, false),
        (-1, 0, 100, false),
        (0, 1, 100, false),
        (0, -1, 100, false),
        (1, 1, 141, true),
        (1, -1, 141, true),
        (-1, 1, 141, true),
        (-1, -1, 141, true),
    ];

    while let Some(Reverse((_, cur))) = open.pop() {
        if cur == goal_i {
            // Rekonstruer.
            let mut path = Vec::new();
            let mut c = goal_i;
            while c != start_i {
                let cx = (c % MAP_W) as i32;
                let cy = (c / MAP_W) as i32;
                path.push(tile_center(cx, cy));
                c = came[c];
                if c == usize::MAX {
                    break;
                }
            }
            path.reverse();
            return path;
        }
        if closed[cur] {
            continue;
        }
        closed[cur] = true;
        let cx = (cur % MAP_W) as i32;
        let cy = (cur / MAP_W) as i32;

        for &(dx, dy, cost, diag) in &dirs {
            let nx = cx + dx;
            let ny = cy + dy;
            if !in_bounds(nx, ny) || blocked[idx(nx, ny)] {
                continue;
            }
            if diag {
                // Ikke kutt hjorner: begge ortogonale naboer ma vaere apne.
                if blocked[idx(cx + dx, cy)] || blocked[idx(cx, cy + dy)] {
                    continue;
                }
            }
            let ni = idx(nx, ny);
            if closed[ni] {
                continue;
            }
            let tentative = g_score[cur].saturating_add(cost);
            if tentative < g_score[ni] {
                g_score[ni] = tentative;
                came[ni] = cur;
                open.push(Reverse((tentative.saturating_add(h(nx, ny)), ni)));
            }
        }
    }
    vec![]
}

// ---------------------------------------------------------------------------
// Enheter
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum UnitKind {
    Rifleman,
    Tank,
    Harvester,
}

impl UnitKind {
    fn name_key(self) -> Key {
        match self {
            UnitKind::Rifleman => Key::UnitRifleman,
            UnitKind::Tank => Key::UnitTank,
            UnitKind::Harvester => Key::UnitHarvester,
        }
    }
    fn hotkey(self) -> &'static str {
        match self {
            UnitKind::Rifleman => "1",
            UnitKind::Tank => "2",
            UnitKind::Harvester => "3",
        }
    }
}

struct UnitStats {
    hp: f32,
    speed: f32,
    range: f32,
    damage: f32,
    fire: f32,
    radius: f32,
    cost: f32,
    build_time: f32,
    sight: i32,
}

fn unit_stats(kind: UnitKind) -> UnitStats {
    match kind {
        UnitKind::Rifleman => UnitStats {
            hp: 45.0,
            speed: 70.0,
            range: 110.0,
            damage: 6.0,
            fire: 0.45,
            radius: 8.0,
            cost: 100.0,
            build_time: 3.0,
            sight: 6,
        },
        UnitKind::Tank => UnitStats {
            hp: 150.0,
            speed: 60.0,
            range: 150.0,
            damage: 14.0,
            fire: 0.7,
            radius: 12.0,
            cost: 500.0,
            build_time: 9.0,
            sight: 6,
        },
        UnitKind::Harvester => UnitStats {
            hp: 120.0,
            speed: 55.0,
            range: 0.0,
            damage: 0.0,
            fire: 1.0,
            radius: 12.0,
            cost: 300.0,
            build_time: 7.0,
            sight: 5,
        },
    }
}

#[derive(Clone, Copy, PartialEq)]
enum HarvState {
    Idle,
    ToOre,
    Mining,
    ToBase,
    Unloading,
    Manual,
}

struct Unit {
    pos: Vec2,
    path: Vec<Vec2>,
    target: Option<Vec2>,
    kind: UnitKind,
    hp: f32,
    max_hp: f32,
    team: u8,
    cooldown: f32,
    selected: bool,
    carrying: f32,
    harv: HarvState,
    work_timer: f32, // nedtelling for lossing
    aggressive: bool, // AI-enhet i angreps-/forsvarsmodus (forfolger mal)
    stuck: f32,       // tid uten fremgang -> bryter vranglas/viker
}

impl Unit {
    fn new(pos: Vec2, team: u8, kind: UnitKind) -> Self {
        let s = unit_stats(kind);
        Unit {
            pos,
            path: Vec::new(),
            target: None,
            kind,
            hp: s.hp,
            max_hp: s.hp,
            team,
            cooldown: 0.0,
            selected: false,
            carrying: 0.0,
            harv: HarvState::Idle,
            work_timer: 0.0,
            aggressive: false,
            stuck: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Bygninger
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum BuildingKind {
    Hq,
    Refinery,
    Factory,
    Wall,   // gjerde: sperrer vei, billig, taler litt
    Turret, // vakttarn: skyter pa fiender
}

impl BuildingKind {
    fn max_hp(self) -> f32 {
        match self {
            BuildingKind::Hq => 800.0,
            BuildingKind::Refinery => 500.0,
            BuildingKind::Factory => 600.0,
            BuildingKind::Wall => 300.0,
            BuildingKind::Turret => 750.0,
        }
    }
    fn radius(self) -> f32 {
        match self {
            BuildingKind::Hq => TILE * 1.6,
            BuildingKind::Wall => TILE * 0.5,
            BuildingKind::Turret => TILE * 0.8,
            _ => TILE * 1.4,
        }
    }
    fn label_key(self) -> Key {
        match self {
            BuildingKind::Hq => Key::BldHqShort,
            BuildingKind::Refinery => Key::BldRefineryShort,
            BuildingKind::Factory => Key::BldFactoryShort,
            BuildingKind::Wall => Key::BldWallShort,
            BuildingKind::Turret => Key::BldTurretShort,
        }
    }
    fn name_key(self) -> Key {
        match self {
            BuildingKind::Hq => Key::BldHq,
            BuildingKind::Refinery => Key::BldRefinery,
            BuildingKind::Factory => Key::BldFactory,
            BuildingKind::Wall => Key::BldWall,
            BuildingKind::Turret => Key::BldTurret,
        }
    }
    fn cost(self) -> f32 {
        match self {
            BuildingKind::Hq => 3000.0,
            BuildingKind::Refinery => 1500.0,
            BuildingKind::Factory => 2000.0,
            BuildingKind::Wall => 100.0,
            BuildingKind::Turret => 800.0,
        }
    }
    // Kampverdier for bygninger som skyter (None = passiv). (skade, rekkevidde, fyringstid, syn)
    fn combat(self) -> Option<(f32, f32, f32, i32)> {
        match self {
            BuildingKind::Turret => Some((34.0, 230.0, 1.1, 9)),
            _ => None,
        }
    }
}

struct Building {
    pos: Vec2,
    kind: BuildingKind,
    hp: f32,
    team: u8,
    selected: bool,
    cooldown: f32,    // for tarn-skyting
    condemned: bool,  // merket for riving av egne enheter (flytt/fjern)
    // Some(gammel_pos): bygges fortsatt og blir IKKE operativ for bygningen pa
    // den gamle posisjonen er revet. None = ferdig/operativ.
    awaiting: Option<Vec2>,
}

impl Building {
    fn new(pos: Vec2, team: u8, kind: BuildingKind) -> Self {
        Building {
            pos,
            kind,
            hp: kind.max_hp(),
            team,
            selected: false,
            cooldown: 0.0,
            condemned: false,
            awaiting: None,
        }
    }
    // Operativ = ferdig bygget (ikke venter pa at en gammel skal rives).
    fn operational(&self) -> bool {
        self.awaiting.is_none()
    }
}

struct Shot {
    from: Vec2,
    to: Vec2,
    team: u8,
    life: f32,
}

// Varmesokende "storkule" fra vakttarn -- flyr mot naermeste fiende og treffer.
struct Projectile {
    pos: Vec2,
    dir: Vec2,
    team: u8,
    damage: f32,
    life: f32,
}

const PROJECTILE_SPEED: f32 = 300.0;
const PROJECTILE_TURN: f32 = 9.0; // hvor raskt den svinger mot malet (homing)
const PROJECTILE_HIT: f32 = 24.0; // trefferadius

#[derive(Default)]
struct Production {
    queue: VecDeque<UnitKind>,
    // En aktiv byggeplass per fabrikk: (enhet, gjenstaende tid). Flere fabrikker
    // -> flere samtidige bygg -> raskere produksjon.
    active: Vec<(UnitKind, f32)>,
}

/// En klynge av sammenhengende malm-ruter (dekomponering: vi tildeler hostere
/// til *felt*, ikke til enkeltruter).
struct OreField {
    centroid: Vec2,
    remaining: f32,
    tiles: Vec<(i32, i32)>,
}

/// Smart tildelings-score for et malmfelt: gjennomstrømning (levert last pr.
/// effektiv rundeturtid), der `predicted_wait` er kotid vi har *forutsagt* vil
/// oppsta ved raffineriet nar hosten kommer tilbake.
///   effektiv rundeturtid = reise_til/fart + hostetid + reise_tilbake/fart + kotid
/// Hoyere score = bedre valg.
fn harvest_score(travel_to: f32, mine_time: f32, travel_back: f32, predicted_wait: f32, delivered: f32) -> f32 {
    let speed = unit_stats(UnitKind::Harvester).speed;
    let round_trip = travel_to / speed + mine_time + travel_back / speed + predicted_wait + 0.001;
    delivered / round_trip
}

// ---------------------------------------------------------------------------
// Spilltilstand
// ---------------------------------------------------------------------------

// Hvilken skjerm vises. Start = nivåvelger/meny, Guide = veiledning, Playing =
// selve spillet. Meny/guide pauser all spill-logikk.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Start,
    Guide,
    Playing,
}

struct Game {
    map: Vec<Terrain>,
    ore: Vec<f32>,
    units: Vec<Unit>,
    buildings: Vec<Building>,
    shots: Vec<Shot>,
    projectiles: Vec<Projectile>,
    confirm_remove: bool, // Fjern krever en ekstra bekreftelse
    credits: [f32; 2],
    prod: Vec<Production>,
    rally: [Vec2; 2],
    explored: Vec<bool>,
    visible: Vec<bool>,
    cam: Vec2,
    zoom: f32,
    drag_start: Option<Vec2>,
    mouse_active: bool,
    last_mouse: Vec2,
    // touch (mobil): to-finger panorering + pinch-zoom
    touch_centroid: Option<Vec2>,
    touch_dist: Option<f32>,
    multitouch: bool,
    // en-finger panorering
    touch1_start: Option<Vec2>,
    touch1_last: Option<Vec2>,
    touch_panning: bool,
    // hoyre museknapp: dra for a panorere (PC); rent klikk = fjern markering
    right_pan_start: Option<Vec2>,
    right_pan_last: Option<Vec2>,
    right_pan_dragged: bool,
    sidebar_open: bool, // burger-bryter (skjul byggmeny pa sma skjermer)
    pan_vel: Vec2,      // kamera-pan-hastighet fra joystick [-1,1]
    enemy_attack_timer: f32,
    enemy_waves: u32, // antall angrepsbolger sendt -> okende styrke
    placing: Option<BuildingKind>, // bygning under plassering (sidebar)
    move_src: Option<Vec2>,        // posisjonen til bygningen som flyttes (ellers ny)
    wall_drag: Option<Vec2>,       // startpunkt nar man drar en gjerde-rekke
    // Some(true) = seier, Some(false) = nederlag, None = spillet pagar.
    outcome: Option<bool>,
    spawn_rot: f32,
    free_build: bool,
    god_mode: bool,
    reveal: bool,
    speed: f32,
    paused: bool,
    lang: Lang, // valgt sprak (standard engelsk)
    // --- Kampanje / niva (les fra levels::LEVELS) ---
    level: usize,           // gjeldende niva (0-basert)
    enemy_income: f32,      // passiv kreditt/sek for fiendelaget
    first_wave: u32,        // forste bolge krever sa mange kampenheter
    wave_growth: u32,       // okning per bolge
    wave_cap: u32,          // tak pa bolgestorrelse
    ai_tick: f32,           // sek mellom AI-beslutninger
    enemy_power: f32,       // hp/skade-multiplikator pa fiende-enheter
    enemy_style: levels::EnemyStyle, // former fiendens enhetsmiks + tint
    level_time: f32,        // spilt tid pa nivaet (score; teller ikke pause)
    level_cheated: bool,    // juks brukt -> ingen tid/score vises
    // --- In-canvas UI (alt i Rust, native-klar -- ingen JS/HTML) ---
    joy_active: bool, // joysticken dras na
    joy_vec: Vec2,    // knott-forskyvning [-1,1] (for tegning)
    dev_open: bool,   // dev-panelet er apent
    dev_warn: bool,   // viser advarsel for dev aktiveres forste gang
    cheater: bool,    // dev er akseptert -> vis "Cheater"-merke
    lang_open: bool,  // sprakvelger-listen er apen
    lang_scroll: f32, // rulling i spraklisten
    queue_open: bool, // produksjonsko-popup (nar byggmeny lukket)
    muted: bool,      // lyd av/pa (dev)
    ui_block: bool,   // pekeren ble brukt av UI denne rammen -> ikke selekter
    ui_press: Vec2,   // posisjon der venstre-trykk startet (UI-dra)
    lang_dragging: bool, // ruller spraklista med fingeren
    ui_init: bool,    // forste-frame-oppsett gjort (sidebar-standard etter skjerm)
    // --- Skjerm / meny ---
    screen: Screen,        // Start (nivameny) / Guide / Playing
    max_unlocked: usize,   // hoyeste lasted opp niva (0-basert); nivaer <= dette + 1 spillbare
    playing_started: bool, // et niva er startet fra menyen -> vis "Tilbake til spillet"
    nav_show: f32,         // nedtelling: vis flytende minikart mens man navigerer
    prev_cam: Vec2,        // forrige frames kamera (for a oppdage navigering)
    rally_show: f32,       // nedtelling: vis samlepunkt-flagget etter at det ble satt
    move_marker: Option<(Vec2, f32)>, // (verdens-punkt, nedtelling) der enheter ble sendt
    confirm_level: Option<usize>, // nivameny: avventer bekreftelse for bytte (mister data)
    touch_device: bool,    // har sett touch-input -> sla av kant-scroll (mobil)
    settings_open: bool,   // settings-panel (lyd/pause) over burgeren er apent
    // Levende kart-forhandsvisning bak nivavelgeren (egen simulering i loop).
    preview: Option<Box<Game>>,
    preview_level: usize,
    preview_time: f32,
}

impl Game {
    fn new() -> Self {
        Self::new_level(0)
    }

    // Bygg et niva fra levels::LEVELS. Terreng, baseplassering, kreditter og
    // vanskelighets-tall kommer fra LevelSpec.
    fn new_level(level: usize) -> Self {
        let spec = levels::get(level);
        set_enemy_tint(spec.enemies[0].style);

        let mut map = levels::gen_map_for(spec);
        // Rydd terrenget under HELE basen -- ikke bare HK, men ogsa rutene der
        // raffineri og fabrikk havner (samme forskyvninger som spawn_base, m=-1
        // for spiller, +1 for fiende). Ellers kan et bygg lande pa malm/fjell
        // utenfor det ryddede feltet (sett pa niva 2: raffineriet la oppa malmen
        // sa harvesteren satte seg fast uten a hente/levere).
        let clear_base = |map: &mut Vec<Terrain>, base: (i32, i32), m: f32| {
            let pts = [
                (base.0, base.1, 3i32), // HK + naerliggende enheter
                (
                    base.0 + (150.0 * m / TILE).round() as i32,
                    base.1 + (20.0 / TILE).round() as i32,
                    2,
                ), // raffineri
                (
                    base.0 + (40.0 * m / TILE).round() as i32,
                    base.1 + (150.0 / TILE).round() as i32,
                    2,
                ), // fabrikk
            ];
            for (cx, cy, rad) in pts {
                for dy in -rad..=rad {
                    for dx in -rad..=rad {
                        let (x, y) = (cx + dx, cy + dy);
                        if x >= 0 && y >= 0 && (x as usize) < MAP_W && (y as usize) < MAP_H {
                            map[y as usize * MAP_W + x as usize] = Terrain::Grass;
                        }
                    }
                }
            }
        };
        clear_base(&mut map, spec.player_base, -1.0);
        for e in spec.enemies {
            clear_base(&mut map, e.pos, 1.0);
        }

        let mut ore = vec![0.0f32; MAP_W * MAP_H];
        for i in 0..map.len() {
            if map[i] == Terrain::Ore {
                ore[i] = ORE_PER_TILE;
            }
        }

        let mut units = Vec::new();
        let mut buildings = Vec::new();
        let pbase = vec2(spec.player_base.0 as f32 * TILE, spec.player_base.1 as f32 * TILE);
        let player_rally = spawn_base(&mut units, &mut buildings, pbase, TEAM_PLAYER, 1.0);
        let mut enemy_rally = pbase;
        for (k, e) in spec.enemies.iter().enumerate() {
            let eb = vec2(e.pos.0 as f32 * TILE, e.pos.1 as f32 * TILE);
            let r = spawn_base(&mut units, &mut buildings, eb, TEAM_ENEMY, spec.enemy_power);
            if k == 0 {
                enemy_rally = r;
            }
        }
        let rally = [player_rally, enemy_rally];

        let mut g = Game {
            map,
            ore,
            units,
            buildings,
            shots: Vec::new(),
            projectiles: Vec::new(),
            confirm_remove: false,
            credits: [spec.player_credits, spec.enemy_credits],
            prod: (0..2).map(|_| Production::default()).collect(),
            rally,
            explored: vec![false; MAP_W * MAP_H],
            visible: vec![false; MAP_W * MAP_H],
            cam: pbase - vec2(440.0, 320.0), // klampes/sentreres uansett ved forste frame
            zoom: 1.0,
            drag_start: None,
            mouse_active: false,
            last_mouse: Vec2::ZERO,
            touch_centroid: None,
            touch_dist: None,
            multitouch: false,
            touch1_start: None,
            touch1_last: None,
            touch_panning: false,
            right_pan_start: None,
            right_pan_last: None,
            right_pan_dragged: false,
            sidebar_open: true,
            pan_vel: Vec2::ZERO,
            enemy_attack_timer: spec.attack_delay,
            enemy_waves: 0,
            placing: None,
            move_src: None,
            wall_drag: None,
            outcome: None,
            spawn_rot: 0.0,
            free_build: false,
            god_mode: false,
            reveal: false,
            speed: 1.0,
            paused: false,
            lang: Lang::En,
            level,
            enemy_income: spec.enemy_income,
            first_wave: spec.first_wave,
            wave_growth: spec.wave_growth,
            wave_cap: spec.wave_cap,
            ai_tick: spec.ai_tick,
            enemy_power: spec.enemy_power,
            enemy_style: spec.enemies[0].style,
            level_time: 0.0,
            level_cheated: false,
            joy_active: false,
            joy_vec: Vec2::ZERO,
            dev_open: false,
            dev_warn: false,
            cheater: false,
            lang_open: false,
            lang_scroll: 0.0,
            queue_open: false,
            muted: false,
            ui_block: false,
            ui_press: Vec2::ZERO,
            lang_dragging: false,
            ui_init: false,
            screen: Screen::Start,
            max_unlocked: 0,
            playing_started: false,
            nav_show: 0.0,
            prev_cam: Vec2::ZERO,
            rally_show: 0.0,
            move_marker: None,
            confirm_level: None,
            touch_device: false,
            settings_open: false,
            preview: None,
            preview_level: 0,
            preview_time: 0.0,
        };
        g.compute_visibility();
        g
    }

    // Last et niva pa nytt (bevarer sprak/cheater/lyd + UI-tilstand over bytte).
    fn load_level(&mut self, level: usize) {
        let (lang, cheater, muted) = (self.lang, self.cheater, self.muted);
        let (sidebar, ui_init) = (self.sidebar_open, self.ui_init);
        let (screen, unlocked, started) = (self.screen, self.max_unlocked, self.playing_started);
        *self = Game::new_level(level.min(levels::count().saturating_sub(1)));
        self.lang = lang;
        self.cheater = cheater;
        self.muted = muted;
        self.sidebar_open = sidebar;
        self.ui_init = ui_init;
        self.screen = screen;
        self.max_unlocked = unlocked;
        self.playing_started = started;
        bridge::set_muted(muted);
    }

    /// Oversett en nokkel til valgt sprak (engelsk fallback).
    fn t(&self, key: Key) -> &'static str {
        i18n::t(self.lang, key)
    }

    fn world_to_screen(&self, p: Vec2) -> Vec2 {
        (p - self.cam) * self.zoom
    }
    fn screen_to_world(&self, p: Vec2) -> Vec2 {
        p / self.zoom + self.cam
    }

    // Bredden pa selve spillflaten (skjerm minus sidebar). Pa smale skjermer
    // (mobil staende) droppes sidebar slik at hele bredden er spillflate.
    fn sidebar_on(&self) -> bool {
        // Burger-bryteren styrer alt na (in-canvas). Pa veldig smale skjermer
        // ville sidebaren spist hele bildet, sa vi krever litt minstebredde.
        self.sidebar_open && screen_width() > 360.0
    }
    fn play_w(&self) -> f32 {
        if self.sidebar_on() {
            (screen_width() - SIDEBAR_W).max(100.0)
        } else {
            screen_width()
        }
    }
    fn in_sidebar(&self, p: Vec2) -> bool {
        self.sidebar_on() && p.x >= self.play_w()
    }
    fn minimap_rect(&self) -> Rect {
        let x = self.play_w() + 8.0;
        let w = SIDEBAR_W - 16.0;
        let h = w * (MAP_H as f32 / MAP_W as f32);
        Rect::new(x, 62.0, w, h) // plass til kreditt-banner over
    }
    // Byggknapper i sidebar: (rute, enhetstype). Stables under minikartet.
    fn build_buttons(&self) -> [(Rect, UnitKind); 3] {
        let mm = self.minimap_rect();
        let x = self.play_w() + 8.0;
        let w = SIDEBAR_W - 16.0;
        let h = 46.0;
        let y0 = mm.y + mm.h + 12.0;
        [
            (Rect::new(x, y0, w, h), UnitKind::Rifleman),
            (Rect::new(x, y0 + h + 8.0, w, h), UnitKind::Tank),
            (Rect::new(x, y0 + 2.0 * (h + 8.0), w, h), UnitKind::Harvester),
        ]
    }

    // Kategorisert bygnings-meny. Returnerer (kategori-overskrifter med y, knapper).
    fn building_menu(&self) -> (Vec<(f32, Key)>, Vec<(Rect, BuildingKind)>) {
        let cats: [(Key, &'static [BuildingKind]); 2] = [
            (Key::CatEconomy, &[BuildingKind::Refinery, BuildingKind::Factory]),
            (Key::CatDefense, &[BuildingKind::Wall, BuildingKind::Turret]),
        ];
        let x = self.play_w() + 8.0;
        let w = SIDEBAR_W - 16.0;
        let h = 38.0;
        let u = self.build_buttons();
        let mut y = u[2].0.y + u[2].0.h + 16.0;
        let mut headers = Vec::new();
        let mut btns = Vec::new();
        for (name, kinds) in cats {
            headers.push((y, name));
            y += 16.0;
            for &k in kinds {
                btns.push((Rect::new(x, y, w, h), k));
                y += h + 6.0;
            }
            y += 6.0;
        }
        (headers, btns)
    }

    // Handlingsknapper for valgt bygning: (Flytt, Fjern). None hvis ingen
    // (ikke-HK) bygning er valgt.
    // (Flytt, Reparer, Fjern). Forankret nederst sa de alltid synes.
    fn building_action_buttons(&self) -> Option<(Rect, Rect, Rect)> {
        self.buildings
            .iter()
            .find(|b| b.selected && b.team == TEAM_PLAYER && b.kind != BuildingKind::Hq)?;
        let x = self.play_w() + 8.0;
        let full = SIDEBAR_W - 16.0;
        let half = (full - 6.0) / 2.0;
        let h = 36.0;
        // Reserver nederste stripe til Dev/sprak-knappene.
        let y2 = screen_height() - 28.0 - h - 34.0; // Fjern (full bredde)
        let y1 = y2 - h - 6.0; // Flytt | Reparer
        Some((
            Rect::new(x, y1, half, h),
            Rect::new(x + half + 6.0, y1, half, h),
            Rect::new(x, y2, full, h),
        ))
    }

    // Reparer valgt bygning UMIDDELBART til full HP mot et gebyr (andel av
    // manglende HP). Live-reparasjon uten forsinkelse.
    fn repair_building(&mut self, idx: usize) -> bool {
        let (maxhp, cost, hp) = {
            let b = &self.buildings[idx];
            (b.kind.max_hp(), b.kind.cost(), b.hp)
        };
        if maxhp - hp <= 1.0 {
            return false; // allerede full
        }
        let fee = ((maxhp - hp) / maxhp) * cost * 0.5;
        if !self.free_build {
            if self.credits[TEAM_PLAYER as usize] < fee {
                return false; // ikke rad
            }
            self.credits[TEAM_PLAYER as usize] -= fee;
        }
        self.buildings[idx].hp = maxhp;
        true
    }

    // Et lite flyttegebyr (10 %), ikke full pris -- billig a omplassere.
    fn move_cost(kind: BuildingKind) -> f32 {
        (kind.cost() * 0.1).max(10.0)
    }

    // Tile-senter under et verdenspunkt (snapper bygninger til rutenettet).
    fn snap_to_tile(p: Vec2) -> Vec2 {
        let tx = (p.x / TILE).floor();
        let ty = (p.y / TILE).floor();
        vec2((tx + 0.5) * TILE, (ty + 0.5) * TILE)
    }

    // Kan en bygning av denne typen sta her? Fotavtrykket ma vaere fremkommelig
    // terreng, innenfor kartet, og ikke overlappe andre bygninger.
    fn can_place_building(&self, kind: BuildingKind, center: Vec2) -> bool {
        let r = kind.radius();
        let minx = ((center.x - r) / TILE).floor() as i32;
        let maxx = ((center.x + r) / TILE).floor() as i32;
        let miny = ((center.y - r) / TILE).floor() as i32;
        let maxy = ((center.y + r) / TILE).floor() as i32;
        for ty in miny..=maxy {
            for tx in minx..=maxx {
                if !in_bounds(tx, ty) || !self.map[ty as usize * MAP_W + tx as usize].passable() {
                    return false;
                }
            }
        }
        // Gjerder kan sta helt inntil hverandre (danne en mur); andre bygninger
        // krever en passasje (~1 rute) sa nye enheter ikke blir sperret inne.
        let margin = if kind == BuildingKind::Wall { 0.0 } else { TILE };
        for b in &self.buildings {
            if b.pos.distance(center) < b.kind.radius() + r + margin {
                return false;
            }
        }
        true
    }

    // Rette ruter (langs dominerende akse) for a dra en gjerde-rekke.
    fn wall_line_tiles(&self, start: Vec2, end: Vec2) -> Vec<Vec2> {
        let sx = (start.x / TILE).floor() as i32;
        let sy = (start.y / TILE).floor() as i32;
        let ex = (end.x / TILE).floor() as i32;
        let ey = (end.y / TILE).floor() as i32;
        let (dx, dy) = (ex - sx, ey - sy);
        let mut tiles = Vec::new();
        if dx.abs() >= dy.abs() {
            let step = if dx >= 0 { 1 } else { -1 };
            let mut x = sx;
            loop {
                tiles.push(tile_center(x, sy));
                if x == ex || tiles.len() > 64 {
                    break;
                }
                x += step;
            }
        } else {
            let step = if dy >= 0 { 1 } else { -1 };
            let mut y = sy;
            loop {
                tiles.push(tile_center(sx, y));
                if y == ey || tiles.len() > 64 {
                    break;
                }
                y += step;
            }
        }
        tiles
    }

    // Bygg en rekke gjerder langs en linje (stopper nar pengene tar slutt).
    fn place_wall_line(&mut self, start: Vec2, end: Vec2) {
        for center in self.wall_line_tiles(start, end) {
            if !self.free_build && self.credits[TEAM_PLAYER as usize] < BuildingKind::Wall.cost() {
                break;
            }
            self.place_building(BuildingKind::Wall, center);
        }
    }

    // Forsok a sette opp en bygning for spilleren. Returnerer true ved suksess.
    fn place_building(&mut self, kind: BuildingKind, center: Vec2) -> bool {
        let center = Self::snap_to_tile(center);
        if !self.can_place_building(kind, center) {
            return false;
        }
        let cost = kind.cost();
        let free = self.free_build;
        if !free {
            if self.credits[TEAM_PLAYER as usize] < cost {
                return false;
            }
            self.credits[TEAM_PLAYER as usize] -= cost;
        }
        self.buildings.push(Building::new(center, TEAM_PLAYER, kind));
        bridge::sound(SND_PLACE);
        true
    }

    // Doem en bygning til riving: egne kampenheter rykker ut og skyter den ned
    // (mer moro enn at den bare forsvinner). Fjernes nar HP naar 0.
    fn condemn_building(&mut self, idx: usize) {
        if idx >= self.buildings.len() {
            return;
        }
        self.buildings[idx].condemned = true;
        self.buildings[idx].selected = false;
        let pos = self.buildings[idx].pos;
        let blocked = self.compute_blocked();
        let mut combat: Vec<usize> = self
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.team == TEAM_PLAYER && unit_stats(u.kind).damage > 0.0)
            .map(|(i, _)| i)
            .collect();
        combat.sort_by(|&a, &b| {
            self.units[a]
                .pos
                .distance(pos)
                .partial_cmp(&self.units[b].pos.distance(pos))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for &i in combat.iter().take(4) {
            self.units[i].path = astar(&blocked, self.units[i].pos, pos);
            self.units[i].target = Some(pos);
        }
    }

    // Flytt en bygning mot et gebyr: ny reises umiddelbart, gammel doemmes til
    // riving av egne tanks.
    fn relocate_building(&mut self, kind: BuildingKind, src: Vec2, dest: Vec2) -> bool {
        let dest = Self::snap_to_tile(dest);
        if !self.can_place_building(kind, dest) {
            return false;
        }
        let cost = Self::move_cost(kind);
        if !self.free_build {
            if self.credits[TEAM_PLAYER as usize] < cost {
                return false;
            }
            self.credits[TEAM_PLAYER as usize] -= cost;
        }
        // Ny bygning reises, men er IKKE operativ for den gamle er revet.
        let mut nb = Building::new(dest, TEAM_PLAYER, kind);
        nb.awaiting = Some(src);
        self.buildings.push(nb);
        if let Some(idx) = self
            .buildings
            .iter()
            .position(|b| b.team == TEAM_PLAYER && !b.condemned && b.operational() && b.pos.distance(src) < 1.0)
        {
            self.condemn_building(idx);
        }
        true
    }

    fn factory_pos(&self, team: u8) -> Option<Vec2> {
        self.buildings
            .iter()
            .find(|b| b.team == team && b.kind == BuildingKind::Factory && b.operational())
            .map(|b| b.pos)
    }

    fn count_buildings(&self, team: u8) -> i32 {
        self.buildings.iter().filter(|b| b.team == team).count() as i32
    }

    fn factory_positions(&self, team: u8) -> Vec<Vec2> {
        self.buildings
            .iter()
            .filter(|b| b.team == team && b.kind == BuildingKind::Factory && b.operational())
            .map(|b| b.pos)
            .collect()
    }

    // Antall ferdige fabrikker = antall samtidige byggeplasser.
    fn factory_count(&self, team: u8) -> usize {
        self.buildings
            .iter()
            .filter(|b| b.team == team && b.kind == BuildingKind::Factory && b.operational())
            .count()
    }

    /// Rutenett der ufremkommelig terreng + bygningsfotavtrykk er blokkert.
    fn compute_blocked(&self) -> Vec<bool> {
        let mut blocked = vec![false; MAP_W * MAP_H];
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                if !self.map[y * MAP_W + x].passable() {
                    blocked[y * MAP_W + x] = true;
                }
            }
        }
        for b in &self.buildings {
            let r = b.kind.radius();
            let minx = ((b.pos.x - r) / TILE).floor() as i32;
            let maxx = ((b.pos.x + r) / TILE).floor() as i32;
            let miny = ((b.pos.y - r) / TILE).floor() as i32;
            let maxy = ((b.pos.y + r) / TILE).floor() as i32;
            for ty in miny..=maxy {
                for tx in minx..=maxx {
                    if in_bounds(tx, ty) {
                        blocked[ty as usize * MAP_W + tx as usize] = true;
                    }
                }
            }
        }
        blocked
    }

    // Som compute_blocked, men markerer ogsa rutene til ANDRE enheter som star
    // i ro eller har satt seg fast -> en fastlast enhet kan da regne ut en ny
    // vei RUNDT klyngen (dynamiske hindre). Egen rute og malruten blokkeres ikke.
    fn blocked_with_units(&self, base: &[bool], self_idx: usize, goal: Vec2) -> Vec<bool> {
        let mut g = base.to_vec();
        let gtx = (goal.x / TILE).floor() as i32;
        let gty = (goal.y / TILE).floor() as i32;
        for (k, u) in self.units.iter().enumerate() {
            if k == self_idx {
                continue;
            }
            // Bare enheter som ikke kommer seg fram er hinder (parkerte/fastlaste).
            let obstacle = u.path.is_empty() || u.stuck > 0.3;
            if !obstacle {
                continue;
            }
            let tx = (u.pos.x / TILE).floor() as i32;
            let ty = (u.pos.y / TILE).floor() as i32;
            if in_bounds(tx, ty) && !(tx == gtx && ty == gty) {
                g[ty as usize * MAP_W + tx as usize] = true;
            }
        }
        g
    }

    fn compute_visibility(&mut self) {
        for v in &mut self.visible {
            *v = false;
        }
        if self.reveal {
            for i in 0..self.visible.len() {
                self.visible[i] = true;
                self.explored[i] = true;
            }
            return;
        }
        let mut srcs: Vec<(Vec2, i32)> = Vec::new();
        for u in &self.units {
            if u.team == TEAM_PLAYER {
                srcs.push((u.pos, unit_stats(u.kind).sight));
            }
        }
        for b in &self.buildings {
            if b.team == TEAM_PLAYER {
                srcs.push((b.pos, 8));
            }
        }
        for (c, r) in srcs {
            let cx = (c.x / TILE).floor() as i32;
            let cy = (c.y / TILE).floor() as i32;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy > r * r {
                        continue;
                    }
                    let x = cx + dx;
                    let y = cy + dy;
                    if in_bounds(x, y) {
                        let i = y as usize * MAP_W + x as usize;
                        self.visible[i] = true;
                        self.explored[i] = true;
                    }
                }
            }
        }
    }

    #[inline]
    fn tile_visible(&self, p: Vec2) -> bool {
        let x = (p.x / TILE).floor() as i32;
        let y = (p.y / TILE).floor() as i32;
        in_bounds(x, y) && self.visible[y as usize * MAP_W + x as usize]
    }

    // Er punktet pa fremkommelig terreng? Brukes for a ikke skyve enheter inn
    // i fjell/vann nar de viker for hverandre.
    #[inline]
    fn passable_world(&self, p: Vec2) -> bool {
        let x = (p.x / TILE).floor() as i32;
        let y = (p.y / TILE).floor() as i32;
        in_bounds(x, y) && self.map[y as usize * MAP_W + x as usize].passable()
    }

    // ----- input -----

    fn handle_camera(&mut self, dt: f32) {
        let mut mv = Vec2::ZERO;
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
            mv.y -= 1.0;
        }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
            mv.y += 1.0;
        }
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
            mv.x -= 1.0;
        }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
            mv.x += 1.0;
        }

        let (mx, my) = mouse_position();
        let mouse = vec2(mx, my);
        if mouse.distance(self.last_mouse) > 0.5 {
            self.mouse_active = true;
        }
        self.last_mouse = mouse;
        // Touch-enhet? (sticky). Pa mobil panorerer man med joysticken, sa kant-
        // scroll skal IKKE vaere aktiv der -- ellers scroller nesten ethvert trykk.
        if !touches().is_empty() {
            self.touch_device = true;
        }

        let inside = mx >= 0.0 && my >= 0.0 && mx <= screen_width() && my <= screen_height();
        // Ikke kant-scroll nar pekeren er over en kontroll/meny (ellers drifter
        // kartet nar man trykker zoom/burger i hjornet) -- og ikke nar den apne
        // byggmenyen ligger under pekeren (man skal kunne velge i menyen i ro).
        if self.mouse_active
            && !self.touch_device
            && inside
            && !self.joy_active
            && !self.point_in_ui(mouse)
            && !self.in_sidebar(mouse)
        {
            // Bredere kant-sone (kun mus) -> man slipper a treffe helt ytterst.
            let edge = 70.0;
            if mx < edge {
                mv.x -= 1.0;
            }
            if mx > screen_width() - edge {
                mv.x += 1.0;
            }
            if my < edge {
                mv.y -= 1.0;
            }
            if my > screen_height() - edge {
                mv.y += 1.0;
            }
        }

        if mv != Vec2::ZERO {
            self.cam += mv.normalize() * 600.0 * dt / self.zoom;
        }
        // Joystick-panorering (mobil): jevn dt-basert bevegelse -> ingen hakking.
        if self.pan_vel != Vec2::ZERO {
            self.cam += self.pan_vel * 900.0 * dt / self.zoom;
        }

        let (_, wheel_y) = mouse_wheel();
        if wheel_y != 0.0 {
            let before = self.screen_to_world(vec2(mx, my));
            self.zoom = (self.zoom * if wheel_y > 0.0 { 1.1 } else { 0.9 }).clamp(0.4, 3.0);
            let after = self.screen_to_world(vec2(mx, my));
            self.cam += before - after;
        }

        // Hoyre museknapp: dra for a panorere kameraet (PC). Et rent klikk uten
        // bevegelse fjerner markeringen (handteres i handle_selection via flagget).
        if is_mouse_button_pressed(MouseButton::Right) {
            self.right_pan_start = Some(mouse);
            self.right_pan_last = Some(mouse);
            self.right_pan_dragged = false;
        }
        if is_mouse_button_down(MouseButton::Right) {
            if let (Some(last), Some(start)) = (self.right_pan_last, self.right_pan_start) {
                let delta = mouse - last;
                if delta.length() > 0.0 {
                    self.cam -= delta / self.zoom; // "grip" kartet og dra
                }
                if mouse.distance(start) > 6.0 {
                    self.right_pan_dragged = true; // ble en dra -> ikke fjern markering
                }
                self.right_pan_last = Some(mouse);
            }
        }

        // Touch (mobil).
        let ts = touches();
        if ts.len() >= 2 {
            // To fingre: panorering + pinch-zoom.
            self.multitouch = true;
            self.touch1_last = None;
            self.touch1_start = None;
            self.touch_panning = false;
            let c = (ts[0].position + ts[1].position) * 0.5;
            let d = ts[0].position.distance(ts[1].position);
            if let (Some(lc), Some(ld)) = (self.touch_centroid, self.touch_dist) {
                self.cam -= (c - lc) / self.zoom;
                if ld > 1.0 {
                    let before = self.screen_to_world(c);
                    self.zoom = (self.zoom * (d / ld)).clamp(0.4, 3.0);
                    let after = self.screen_to_world(c);
                    self.cam += before - after;
                }
            }
            self.touch_centroid = Some(c);
            self.touch_dist = Some(d);
        } else if ts.len() == 1 {
            // En finger: markering/flytting (default). Kamera panoreres med
            // joysticken (HTML-overlay) -- ikke med fingeren lenger.
            self.multitouch = false;
            self.touch_centroid = None;
            self.touch_dist = None;
            self.touch_panning = false;
            self.touch1_start = None;
            self.touch1_last = None;
        } else {
            self.multitouch = false;
            self.touch_centroid = None;
            self.touch_dist = None;
            self.touch1_start = None;
            self.touch1_last = None;
        }

        self.clamp_camera();
    }

    fn clamp_camera(&mut self) {
        // Litt luft rundt kartet sa man kan dra forbi kanten (basen klistrer seg
        // ikke helt mot skjermkanten). 6 ruter overscroll i hver retning.
        let margin = 6.0 * TILE;
        let map_px = vec2(MAP_W as f32 * TILE, MAP_H as f32 * TILE);
        let view = vec2(self.play_w(), screen_height()) / self.zoom;
        let min_x = -margin;
        let min_y = -margin;
        let max_x = map_px.x - view.x + margin;
        let max_y = map_px.y - view.y + margin;
        self.cam.x = if max_x > min_x {
            self.cam.x.clamp(min_x, max_x)
        } else {
            (map_px.x - view.x) / 2.0
        };
        self.cam.y = if max_y > min_y {
            self.cam.y.clamp(min_y, max_y)
        } else {
            (map_px.y - view.y) / 2.0
        };
    }

    fn clear_selection(&mut self) {
        for u in &mut self.units {
            u.selected = false;
        }
        for b in &mut self.buildings {
            b.selected = false;
        }
        self.confirm_remove = false; // ny markering nullstiller Fjern-bekreftelse
    }

    #[allow(dead_code)] // brukes i tester
    fn has_player_selection(&self) -> bool {
        self.units.iter().any(|u| u.selected && u.team == TEAM_PLAYER)
            || self.buildings.iter().any(|b| b.selected && b.team == TEAM_PLAYER)
    }

    // Gi flyttordre til markerte enheter (og sett samlepunkt hvis fabrikk er
    // markert). Brukes av bade hoyreklikk og tapp-for-a-flytte.
    /// Pakkede formasjonsfelt rundt et senter (kvadratisk rutenett). Gir hver
    /// enhet sitt EGET mal slik at en gruppe ikke slass om samme felt.
    fn formation_slots(center: Vec2, n: usize, spacing: f32) -> Vec<Vec2> {
        let cols = (n as f32).sqrt().ceil().max(1.0) as i32;
        let rows = ((n as f32) / cols as f32).ceil().max(1.0) as i32;
        let mut v = Vec::with_capacity(n);
        for r in 0..rows {
            for c in 0..cols {
                if v.len() >= n {
                    break;
                }
                let x = (c as f32 - (cols - 1) as f32 / 2.0) * spacing;
                let y = (r as f32 - (rows - 1) as f32 / 2.0) * spacing;
                v.push(center + vec2(x, y));
            }
        }
        v
    }

    /// Ville enhet `idx` overlappet en annen enhet om den sto pa `p`?
    fn unit_overlaps_at(&self, idx: usize, p: Vec2) -> bool {
        let r = unit_stats(self.units[idx].kind).radius;
        self.units.iter().enumerate().any(|(k, o)| {
            k != idx && o.pos.distance(p) < r + unit_stats(o.kind).radius
        })
    }

    // Finn et ledig felt naer samlepunktet sa nye enheter ikke stables oppa
    // hverandre (ringsok utover). Ellers klumper hele produksjonen seg pa ETT
    // punkt og enheter inne i klyngen kommer seg ikke ut.
    fn free_rally_slot(&self, team: u8, center: Vec2, blocked: &[bool]) -> Vec2 {
        let spacing = unit_stats(UnitKind::Tank).radius * 2.2;
        for ring in 0..10i32 {
            let count = if ring == 0 { 1 } else { ring * 6 };
            for k in 0..count {
                let ang = (k as f32 / count as f32) * std::f32::consts::TAU + ring as f32 * 0.6;
                let p = if ring == 0 {
                    center
                } else {
                    center + vec2(ang.cos(), ang.sin()) * ring as f32 * spacing
                };
                let (tx, ty) = ((p.x / TILE).floor() as i32, (p.y / TILE).floor() as i32);
                if !in_bounds(tx, ty) || blocked[ty as usize * MAP_W + tx as usize] {
                    continue;
                }
                if !self.passable_world(p) {
                    continue;
                }
                let occupied = self
                    .units
                    .iter()
                    .any(|u| u.team == team && u.pos.distance(p) < spacing * 0.85);
                if !occupied {
                    return p;
                }
            }
        }
        center
    }

    fn move_selected(&mut self, dest: Vec2) {
        let factory_selected = self
            .buildings
            .iter()
            .any(|b| b.selected && b.kind == BuildingKind::Factory && b.team == TEAM_PLAYER);
        if factory_selected {
            self.rally[TEAM_PLAYER as usize] = dest;
            self.rally_show = 1.0;
        }
        let selected: Vec<usize> = self
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.selected && u.team == TEAM_PLAYER)
            .map(|(i, _)| i)
            .collect();
        if selected.is_empty() {
            return;
        }
        self.move_marker = Some((dest, 0.8)); // vis "kjor hit"-merke en kort stund
        let blocked = self.compute_blocked();
        let n = selected.len();
        if n == 1 {
            let i = selected[0];
            self.units[i].path = astar(&blocked, self.units[i].pos, dest);
            self.units[i].target = Some(dest);
            if self.units[i].kind == UnitKind::Harvester {
                self.units[i].harv = HarvState::Manual;
            }
            return;
        }
        // Gi hver enhet sitt eget felt; tildel gradig naermeste ledige felt
        // (minst kryssing) sa de fordeler seg pent rundt malet.
        let spacing = 1.15 * TILE;
        let slots = Self::formation_slots(dest, n, spacing);
        let mut used = vec![false; slots.len()];
        for &i in &selected {
            let upos = self.units[i].pos;
            let mut best: Option<usize> = None;
            let mut bd = f32::MAX;
            for (s, sp) in slots.iter().enumerate() {
                if used[s] {
                    continue;
                }
                let d = upos.distance(*sp);
                if d < bd {
                    bd = d;
                    best = Some(s);
                }
            }
            let d = best.map(|s| {
                used[s] = true;
                slots[s]
            }).unwrap_or(dest);
            self.units[i].path = astar(&blocked, upos, d);
            self.units[i].target = Some(d);
            if self.units[i].kind == UnitKind::Harvester {
                self.units[i].harv = HarvState::Manual;
            }
        }
    }

    fn handle_keys(&mut self) {
        if is_key_pressed(KeyCode::Key1) {
            self.build(TEAM_PLAYER, UnitKind::Rifleman);
        }
        if is_key_pressed(KeyCode::Key2) {
            self.build(TEAM_PLAYER, UnitKind::Tank);
        }
        if is_key_pressed(KeyCode::Key3) {
            self.build(TEAM_PLAYER, UnitKind::Harvester);
        }
        if is_key_pressed(KeyCode::Escape) {
            self.placing = None; // avbryt bygningsplassering / flytting
            self.move_src = None;
            self.wall_drag = None;
        }
    }

    fn handle_selection(&mut self) {
        // Pekeren ble brukt av in-canvas UI (joystick/knapper/paneler) -> ikke
        // selekter i verden.
        if self.joy_active || self.ui_block {
            self.drag_start = None;
            return;
        }
        // Under panorering (en- eller to-finger) skal vi ikke selektere.
        if self.multitouch || self.touch_panning {
            self.drag_start = None;
            if is_mouse_button_released(MouseButton::Left) {
                self.touch_panning = false; // forbruk slik at trykk etterpa kan velge
            }
            return;
        }

        let (mx, my) = mouse_position();
        let mouse = vec2(mx, my);

        // --- Sidebar / minikart: fanger klikk for de nar verden ---
        if self.in_sidebar(mouse) {
            self.drag_start = None;
            // (Minikartet er kun visning -- trykk/dra hopper ikke lenger kamera,
            //  det utlostes utilsiktet nar man brukte burger/menyen.)
            // Trykk pa knapp -> produksjon / start plassering / flytt / fjern.
            if is_mouse_button_pressed(MouseButton::Left) {
                for (rect, kind) in self.build_buttons() {
                    if rect.contains(mouse) {
                        self.build(TEAM_PLAYER, kind);
                    }
                }
                let (_, bbtns) = self.building_menu();
                for (rect, kind) in bbtns {
                    if rect.contains(mouse) {
                        // Veksle: trykk samme igjen avbryter plasseringen.
                        self.placing = if self.placing == Some(kind) { None } else { Some(kind) };
                        self.move_src = None;
                    }
                }
                if let Some((flytt, reparer, fjern)) = self.building_action_buttons() {
                    let sel = self
                        .buildings
                        .iter()
                        .position(|b| b.selected && b.team == TEAM_PLAYER && b.kind != BuildingKind::Hq);
                    if let Some(idx) = sel {
                        if flytt.contains(mouse) {
                            self.placing = Some(self.buildings[idx].kind);
                            self.move_src = Some(self.buildings[idx].pos);
                            self.confirm_remove = false;
                        } else if reparer.contains(mouse) {
                            self.repair_building(idx);
                            self.confirm_remove = false;
                        } else if fjern.contains(mouse) {
                            // Forste trykk arming, andre trykk bekrefter.
                            if self.confirm_remove {
                                self.credits[TEAM_PLAYER as usize] += self.buildings[idx].kind.cost() * 0.5;
                                self.condemn_building(idx);
                                self.confirm_remove = false;
                            } else {
                                self.confirm_remove = true;
                            }
                        }
                    }
                }
            }
            return;
        }

        // --- Bygningsplassering / flytting: tapp/klikk pa kartet ---
        if let Some(kind) = self.placing {
            if is_mouse_button_pressed(MouseButton::Right) {
                self.placing = None; // hoyreklikk avbryter
                self.move_src = None;
                self.wall_drag = None;
                return;
            }
            let w = self.screen_to_world(mouse);
            if kind == BuildingKind::Wall && self.move_src.is_none() {
                // Gjerde: dra for a bygge en hel rekke. Blir i modus for flere.
                if is_mouse_button_pressed(MouseButton::Left) {
                    self.wall_drag = Some(w);
                }
                if is_mouse_button_released(MouseButton::Left) {
                    if let Some(start) = self.wall_drag.take() {
                        self.place_wall_line(start, w);
                    }
                }
            } else if is_mouse_button_pressed(MouseButton::Left) {
                let ok = match self.move_src {
                    Some(src) => self.relocate_building(kind, src, w),
                    None => self.place_building(kind, w),
                };
                if ok {
                    self.placing = None;
                    self.move_src = None;
                }
            }
            self.drag_start = None;
            return; // ingen vanlig seleksjon mens vi plasserer
        }

        if is_mouse_button_pressed(MouseButton::Left) {
            // Lagre i VERDENS-koordinat -> boksen forankres til kartet og dekker
            // stadig storre omrade nar kameraet panorerer under draget.
            self.drag_start = Some(self.screen_to_world(mouse));
        }

        if is_mouse_button_released(MouseButton::Left) {
            if let Some(start) = self.drag_start.take() {
                if (mouse - self.world_to_screen(start)).length() < 6.0 {
                    let w = self.screen_to_world(mouse);
                    // Hva er under markoren?
                    let mut hit_unit: Option<usize> = None;
                    let mut bd = f32::MAX;
                    for (i, u) in self.units.iter().enumerate() {
                        if u.team != TEAM_PLAYER {
                            continue;
                        }
                        let d = u.pos.distance(w);
                        if d < bd && d < unit_stats(u.kind).radius + 14.0 {
                            bd = d;
                            hit_unit = Some(i);
                        }
                    }
                    let hit_bld = self
                        .buildings
                        .iter()
                        .position(|b| b.team == TEAM_PLAYER && b.pos.distance(w) < b.kind.radius());

                    let has_unit_sel =
                        self.units.iter().any(|u| u.selected && u.team == TEAM_PLAYER);
                    let factory_sel = self.buildings.iter().any(|b| {
                        b.selected && b.team == TEAM_PLAYER && b.kind == BuildingKind::Factory
                    });
                    if let Some(i) = hit_unit {
                        // Trykk pa egen enhet -> velg den OG lukk byggmenyen
                        // (man bygger ikke nar man styrer enheter).
                        self.clear_selection();
                        self.units[i].selected = true;
                        self.sidebar_open = false;
                    } else if let Some(i) = hit_bld {
                        // Trykk pa egen bygning -> velg den OG apne byggmenyen
                        // (sa man ser produksjon / reparer / flytt / fjern).
                        self.clear_selection();
                        self.buildings[i].selected = true;
                        self.sidebar_open = true;
                    } else if has_unit_sel {
                        // Tomt punkt med enheter markert -> flytt dit (tapp-for-flytt).
                        // (move_selected setter ogsa samlepunkt hvis fabrikk er valgt.)
                        self.move_selected(w);
                    } else if factory_sel {
                        // Fabrikk valgt + tomt punkt -> sett samlepunkt der, og lukk.
                        self.rally[TEAM_PLAYER as usize] = w;
                        self.rally_show = 1.0; // vis flagget ~1 s selv om menyen lukkes
                        self.clear_selection();
                        self.sidebar_open = false;
                    } else {
                        // Tomt punkt (ingen enheter) -> avmarkér og lukk byggmenyen.
                        self.clear_selection();
                        self.sidebar_open = false;
                    }
                } else {
                    let a = start; // allerede verdens-koordinat
                    let b = self.screen_to_world(mouse);
                    let min = a.min(b);
                    let max = a.max(b);
                    for bl in &mut self.buildings {
                        bl.selected = false;
                    }
                    for u in &mut self.units {
                        u.selected = u.team == TEAM_PLAYER
                            && u.pos.x >= min.x
                            && u.pos.x <= max.x
                            && u.pos.y >= min.y
                            && u.pos.y <= max.y;
                    }
                }
            }
        }

        if is_mouse_button_released(MouseButton::Right) {
            // Rent hoyreklikk (uten dra) fjerner markeringen OG lukker byggmenyen.
            // Var det en dra, panorerte vi kameraet i stedet -> behold alt.
            if !self.right_pan_dragged {
                self.clear_selection();
                self.sidebar_open = false;
            }
            self.right_pan_start = None;
            self.right_pan_last = None;
            self.right_pan_dragged = false;
        }
    }

    // ----- bygging / produksjon -----

    fn build(&mut self, team: u8, kind: UnitKind) {
        if self.factory_pos(team).is_none() {
            return;
        }
        let cost = unit_stats(kind).cost;
        let free = self.free_build && team == TEAM_PLAYER;
        if !free {
            if self.credits[team as usize] < cost {
                return;
            }
            self.credits[team as usize] -= cost;
        }
        self.prod[team as usize].queue.push_back(kind);
    }

    fn nearest_refinery(&self, team: u8, from: Vec2) -> Option<Vec2> {
        self.buildings
            .iter()
            .filter(|b| b.team == team && b.kind == BuildingKind::Refinery && b.operational())
            .map(|b| b.pos)
            .min_by(|a, b| {
                a.distance(from)
                    .partial_cmp(&b.distance(from))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Klyng sammenhengende malm-ruter til felt (flomfyll, 8-naboer).
    fn ore_fields(&self) -> Vec<OreField> {
        let mut visited = vec![false; MAP_W * MAP_H];
        let mut fields = Vec::new();
        for sy in 0..MAP_H {
            for sx in 0..MAP_W {
                let si = sy * MAP_W + sx;
                if self.ore[si] <= 0.0 || visited[si] {
                    continue;
                }
                let mut stack = vec![(sx as i32, sy as i32)];
                visited[si] = true;
                let mut tiles = Vec::new();
                let (mut sum, mut cx, mut cy) = (0.0f32, 0.0f32, 0.0f32);
                while let Some((x, y)) = stack.pop() {
                    let i = y as usize * MAP_W + x as usize;
                    tiles.push((x, y));
                    sum += self.ore[i];
                    cx += (x as f32 + 0.5) * TILE;
                    cy += (y as f32 + 0.5) * TILE;
                    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)] {
                        let (nx, ny) = (x + dx, y + dy);
                        if in_bounds(nx, ny) {
                            let ni = ny as usize * MAP_W + nx as usize;
                            if !visited[ni] && self.ore[ni] > 0.0 {
                                visited[ni] = true;
                                stack.push((nx, ny));
                            }
                        }
                    }
                }
                let n = tiles.len() as f32;
                fields.push(OreField {
                    centroid: vec2(cx / n, cy / n),
                    remaining: sum,
                    tiles,
                });
            }
        }
        fields
    }

    /// Velg hvilket malmfelt en host bor til, basert pa reisetid + hostetid +
    /// trengsel. Returnerer naermeste gjenvaerende malm-rute i det beste feltet.
    /// Rangering bruker billig euklidsk avstand; eksakt A* kjores kun for
    /// vinneren (av kalleren) — sa vi slipper a bygge en full kostmatrise.
    /// Forutsagt ankomsttid (sekunder fra na) til hostens raffineri, gitt dens
    /// naavaerende oppdrag. None for hostere som ikke er i en host-syklus.
    fn harvester_refinery_eta(&self, u: &Unit) -> Option<(f32, Vec2)> {
        let speed = unit_stats(UnitKind::Harvester).speed;
        let r = self.nearest_refinery(u.team, u.pos)?;
        let eta = match u.harv {
            HarvState::Unloading => u.work_timer, // allerede ved dokken
            HarvState::ToBase => u.pos.distance(r) / speed,
            HarvState::Mining => {
                let remaining = (HARVEST_CAPACITY - u.carrying).max(0.0) / HARVEST_RATE;
                remaining + u.pos.distance(r) / speed
            }
            HarvState::ToOre => {
                let t = u.target.unwrap_or(u.pos);
                u.pos.distance(t) / speed + HARVEST_CAPACITY / HARVEST_RATE + t.distance(r) / speed
            }
            _ => return None, // Idle / Manual: ukjent plan
        };
        Some((eta, r))
    }

    fn assign_harvester(&self, team: u8, pos: Vec2, self_idx: usize) -> Option<Vec2> {
        let fields = self.ore_fields();
        if fields.is_empty() {
            return None;
        }
        let speed = unit_stats(UnitKind::Harvester).speed;
        let cap = HARVEST_CAPACITY;

        // Forhandsberegn andre hosteres forutsagte ankomst til raffineriet.
        let mut others: Vec<(f32, Vec2)> = Vec::new();
        for (j, u) in self.units.iter().enumerate() {
            if j == self_idx || u.team != team || u.kind != UnitKind::Harvester {
                continue;
            }
            if let Some(e) = self.harvester_refinery_eta(u) {
                others.push(e);
            }
        }

        let mut best: Option<usize> = None;
        let mut best_score = f32::MIN;
        for (k, f) in fields.iter().enumerate() {
            let travel_to = pos.distance(f.centroid);
            let rf = self.nearest_refinery(team, f.centroid).unwrap_or(f.centroid);
            let back = f.centroid.distance(rf);
            let delivered = cap.min(f.remaining);
            let mine = delivered / HARVEST_RATE;
            // Nar ville JEG vaere tilbake ved raffineriet?
            let h_return = travel_to / speed + mine + back / speed;
            // Forutsi ko: andre som lander pa SAMME raffineri i samme servicevindu.
            let mut competitors = 0u32;
            for (eta, r) in &others {
                if r.distance(rf) < 1.0 && (eta - h_return).abs() < UNLOAD_TIME {
                    competitors += 1;
                }
            }
            let predicted_wait = competitors as f32 * UNLOAD_TIME;
            let score = harvest_score(travel_to, mine, back, predicted_wait, delivered);
            if score > best_score {
                best_score = score;
                best = Some(k);
            }
        }

        // Hvilke malm-ruter sikter andre hostere alt mot? Spre oss utover feltet
        // sa flere ikke konvergerer mot samme rute (gir ulike veier inn/ut).
        let claimed: Vec<Vec2> = self
            .units
            .iter()
            .enumerate()
            .filter(|(j, u)| *j != self_idx && u.team == team && u.kind == UnitKind::Harvester)
            .filter_map(|(_, u)| match u.harv {
                HarvState::ToOre => u.target,
                HarvState::Mining => Some(u.pos),
                _ => None,
            })
            .collect();

        let f = &fields[best?];
        let mut bt = f.tiles[0];
        let mut bd = f32::MAX;
        for &(x, y) in &f.tiles {
            let c = tile_center(x, y);
            let taken = claimed.iter().any(|p| p.distance(c) < TILE * 1.5);
            // Naermeste ledige rute; opptatte ruter straffes men velges om alt er opptatt.
            let d = c.distance_squared(pos) + if taken { 1.0e9 } else { 0.0 };
            if d < bd {
                bd = d;
                bt = (x, y);
            }
        }
        Some(tile_center(bt.0, bt.1))
    }

    // ----- fiende-AI (testbare beslutninger) -----

    fn count_units(&self, team: u8, kind: UnitKind) -> usize {
        self.units.iter().filter(|u| u.team == team && u.kind == kind).count()
    }

    fn hq_pos(&self, team: u8) -> Option<Vec2> {
        self.buildings
            .iter()
            .find(|b| b.team == team && b.kind == BuildingKind::Hq)
            .map(|b| b.pos)
    }

    // Hva bor fienden bygge nest? Okonomi forst (nok hostere), sa haer.
    // Returnerer None hvis ingenting har rad / ingen fabrikk.
    fn enemy_should_build(&self) -> Option<UnitKind> {
        if self.factory_pos(TEAM_ENEMY).is_none() {
            return None;
        }
        let credits = self.credits[TEAM_ENEMY as usize];
        let harvesters = self.count_units(TEAM_ENEMY, UnitKind::Harvester);
        // Bygg opp okonomien forst.
        if harvesters < ENEMY_DESIRED_HARVESTERS && credits >= unit_stats(UnitKind::Harvester).cost {
            return Some(UnitKind::Harvester);
        }
        let tank = unit_stats(UnitKind::Tank).cost;
        let rifle = unit_stats(UnitKind::Rifleman).cost;
        // Stilen former enhetsmiksen.
        match self.enemy_style {
            // Sverm: stort sett billig infanteri.
            levels::EnemyStyle::Swarm => {
                if credits >= rifle {
                    Some(UnitKind::Rifleman)
                } else {
                    None
                }
            }
            // Tank-tung: spar opp til stridsvogn; bygg infanteri bare nar langt unna.
            levels::EnemyStyle::Armor => {
                if credits >= tank {
                    Some(UnitKind::Tank)
                } else if credits >= rifle && credits < tank * 0.6 {
                    Some(UnitKind::Rifleman)
                } else {
                    None
                }
            }
            // Balansert: stridsvogn om rad, ellers infanteri (dagens logikk).
            levels::EnemyStyle::Balanced => {
                if credits >= tank {
                    Some(UnitKind::Tank)
                } else if credits >= rifle {
                    Some(UnitKind::Rifleman)
                } else {
                    None
                }
            }
        }
    }

    // Storrelsen pa neste angrepsbolge (vokser for hver sendt bolge).
    fn enemy_wave_size(&self) -> usize {
        (self.first_wave + self.enemy_waves * self.wave_growth).min(self.wave_cap) as usize
    }

    // Indekser til fiendens kampenheter (ikke hostere).
    fn enemy_combat_units(&self) -> Vec<usize> {
        self.units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.team == TEAM_ENEMY && u.kind != UnitKind::Harvester)
            .map(|(i, _)| i)
            .collect()
    }

    // Spillerenheter som truer fiendens base (innenfor forsvarsradius av HK).
    fn threats_near_enemy_base(&self) -> Vec<usize> {
        let hq = match self.hq_pos(TEAM_ENEMY) {
            Some(p) => p,
            None => return Vec::new(),
        };
        self.units
            .iter()
            .enumerate()
            .filter(|(_, u)| {
                u.team == TEAM_PLAYER
                    && unit_stats(u.kind).damage > 0.0
                    && u.pos.distance(hq) < ENEMY_DEFEND_RADIUS
            })
            .map(|(i, _)| i)
            .collect()
    }

    // Naermeste spiller-mal (enhet eller bygning) fra et punkt -- styrer
    // forfolging: forsvarere/bygninger nedkjempes etter avstand.
    fn nearest_player_target(&self, from: Vec2) -> Option<Vec2> {
        let mut best: Option<Vec2> = None;
        let mut bd = f32::MAX;
        for u in self.units.iter().filter(|u| u.team == TEAM_PLAYER) {
            let d = u.pos.distance(from);
            if d < bd {
                bd = d;
                best = Some(u.pos);
            }
        }
        for b in self.buildings.iter().filter(|b| b.team == TEAM_PLAYER) {
            let d = (b.pos.distance(from) - b.kind.radius()).max(0.0);
            if d < bd {
                bd = d;
                best = Some(b.pos);
            }
        }
        best
    }

    // Tar AI-beslutninger: forsvar basen ved trussel, ellers samle en hær
    // og send en bolge nar den er stor nok. Aggressive enheter forfolger mal.
    fn enemy_ai_decide(&mut self) {
        let combat = self.enemy_combat_units();
        if combat.is_empty() {
            return;
        }
        // Forsvar: trusler ved basen -> hjemmeenheter blir aggressive og snur.
        if !self.threats_near_enemy_base().is_empty() {
            let hq = self.hq_pos(TEAM_ENEMY);
            for &i in &combat {
                let near_home = hq.map_or(true, |h| self.units[i].pos.distance(h) < ENEMY_DEFEND_RADIUS * 1.8);
                if near_home {
                    self.units[i].aggressive = true;
                    self.units[i].path.clear(); // tving ny mal-soking (naermeste = trusselen)
                }
            }
            return;
        }
        // Offensiv: vent til reserven (ikke-aggressive) er stor nok, send sa bolge.
        let reserve: Vec<usize> = combat.iter().copied().filter(|&i| !self.units[i].aggressive).collect();
        if reserve.len() >= self.enemy_wave_size() {
            for &i in &reserve {
                self.units[i].aggressive = true;
            }
            self.enemy_waves += 1;
        }
    }

    // ----- simulering -----

    fn update(&mut self, dt_real: f32) {
        self.compute_visibility();

        if self.outcome.is_some() {
            return;
        }
        let dt = if self.paused { 0.0 } else { dt_real * self.speed };
        if dt == 0.0 {
            return;
        }
        // Score = spilt tid pa nivaet (kun aktiv tid, ikke pause/seier).
        self.level_time += dt_real;

        let blocked = self.compute_blocked();

        self.credits[TEAM_ENEMY as usize] += self.enemy_income * dt;

        // Produksjon: en byggeplass per fabrikk, kjorer parallelt.
        for team in 0..2 {
            let slots = self.factory_count(team as u8);
            let mut done: Vec<UnitKind> = Vec::new();
            {
                let p = &mut self.prod[team];
                // Juster antall aktive bygg til antall fabrikker (mistet fabrikk
                // -> bygg legges tilbake fremst i koen).
                while p.active.len() > slots {
                    if let Some((k, _)) = p.active.pop() {
                        p.queue.push_front(k);
                    }
                }
                // Fyll ledige byggeplasser fra koen.
                while p.active.len() < slots {
                    match p.queue.pop_front() {
                        Some(k) => p.active.push((k, unit_stats(k).build_time)),
                        None => break,
                    }
                }
                // Tell ned alle aktive bygg; samle de ferdige.
                let mut i = 0;
                while i < p.active.len() {
                    p.active[i].1 -= dt;
                    if p.active[i].1 <= 0.0 {
                        done.push(p.active[i].0);
                        p.active.remove(i);
                    } else {
                        i += 1;
                    }
                }
            }
            // Spawn ferdige enheter, fordelt pa fabrikkene.
            let facs = self.factory_positions(team as u8);
            for (idx, kind) in done.into_iter().enumerate() {
                let fp = if facs.is_empty() { continue } else { facs[idx % facs.len()] };
                self.spawn_rot += 1.3;
                let off = vec2(self.spawn_rot.cos(), self.spawn_rot.sin().abs()) * 70.0;
                // Spawn pa naermeste FREMKOMMELIGE rute, sa enheten ikke havner inni
                // en bygning og blir sittende fast.
                let want = fp + off;
                let (tx, ty) = ((want.x / TILE).floor() as i32, (want.y / TILE).floor() as i32);
                let spawn_pos = nearest_free(&blocked, tx, ty).map(|(x, y)| tile_center(x, y)).unwrap_or(want);
                let mut u = Unit::new(spawn_pos, team as u8, kind);
                if team as u8 == TEAM_ENEMY && self.enemy_power != 1.0 {
                    u.hp *= self.enemy_power;
                    u.max_hp *= self.enemy_power;
                }
                if kind != UnitKind::Harvester {
                    // Eget ledig felt naer samlepunktet -> ingen pile-up.
                    let slot = self.free_rally_slot(team as u8, self.rally[team], &blocked);
                    u.path = astar(&blocked, u.pos, slot);
                    u.target = Some(slot);
                }
                self.units.push(u);
                if team as u8 == TEAM_PLAYER {
                    bridge::sound(SND_READY);
                }
            }
        }

        // ----- Fiende-AI -----
        // Kontinuerlig produksjon: okonomi forst, sa haer (erstatter tap).
        if self.prod[TEAM_ENEMY as usize].queue.is_empty() {
            if let Some(kind) = self.enemy_should_build() {
                self.build(TEAM_ENEMY, kind);
            }
        }
        // Beslutninger med jevne mellomrom (forsvar + hærsamling + forfolging).
        self.enemy_attack_timer -= dt;
        if self.enemy_attack_timer <= 0.0 {
            self.enemy_attack_timer = self.ai_tick;
            self.enemy_ai_decide();
            // Aggressive enheter som har stoppet -> finn nytt mal (forfolging).
            for i in self.enemy_combat_units() {
                if self.units[i].aggressive && self.units[i].path.is_empty() {
                    let pos = self.units[i].pos;
                    if let Some(t) = self.nearest_player_target(pos) {
                        let jit = vec2(hash2(pos.x as i32, i as i32) * 60.0 - 30.0, 0.0);
                        self.units[i].path = astar(&blocked, pos, t + jit);
                        self.units[i].target = Some(t);
                    }
                }
            }
        }

        // Hoster-AI.
        // Hvilke raffinerier er opptatt av lossing akkurat na (en host om gangen).
        let mut unloading_at: Vec<Vec2> = self
            .units
            .iter()
            .filter(|u| u.kind == UnitKind::Harvester && u.harv == HarvState::Unloading)
            .filter_map(|u| u.target)
            .collect();
        let n = self.units.len();
        for i in 0..n {
            if self.units[i].kind != UnitKind::Harvester {
                continue;
            }
            let pos = self.units[i].pos;
            let team = self.units[i].team;
            match self.units[i].harv {
                HarvState::Manual => {
                    if self.units[i].path.is_empty() {
                        self.units[i].harv = HarvState::Idle;
                    }
                }
                HarvState::Idle => {
                    if self.units[i].carrying >= HARVEST_CAPACITY {
                        self.units[i].harv = HarvState::ToBase;
                    } else if let Some(ore) = self.assign_harvester(team, pos, i) {
                        self.units[i].path = astar(&blocked, pos, ore);
                        self.units[i].target = Some(ore);
                        self.units[i].harv = HarvState::ToOre;
                    } else if self.units[i].carrying > 0.0 {
                        // Ingen malm igjen pa kartet, men vi har last -> lever den.
                        self.units[i].harv = HarvState::ToBase;
                    }
                }
                HarvState::ToOre => {
                    if self.units[i].path.is_empty() {
                        self.units[i].harv = HarvState::Mining;
                        self.units[i].target = None;
                    }
                }
                HarvState::Mining => {
                    let idx = {
                        let tx = (pos.x / TILE).floor() as i32;
                        let ty = (pos.y / TILE).floor() as i32;
                        if in_bounds(tx, ty) {
                            Some(ty as usize * MAP_W + tx as usize)
                        } else {
                            None
                        }
                    };
                    let mut still_ore = false;
                    if let Some(idx) = idx {
                        if self.ore[idx] > 0.0 {
                            let take = (HARVEST_RATE * dt).min(self.ore[idx]);
                            self.ore[idx] -= take;
                            self.units[i].carrying += take;
                            if self.ore[idx] <= 0.0 {
                                self.map[idx] = Terrain::Grass;
                            }
                            still_ore = true;
                        }
                    }
                    if self.units[i].carrying >= HARVEST_CAPACITY {
                        self.units[i].harv = HarvState::ToBase;
                        self.units[i].path.clear();
                    } else if !still_ore {
                        // Ruten er tom -> finn et nytt felt sa lasten fylles helt
                        // opp for retur (i stedet for a kjore hjem halvfull).
                        self.units[i].harv = HarvState::Idle;
                        self.units[i].path.clear();
                    }
                }
                HarvState::ToBase => {
                    match self.nearest_refinery(team, pos) {
                        Some(r) => {
                            // Romslig: pathfinding kan bare na naermeste apne rute, som ved
                            // tett pakkede bygninger kan vaere et par ruter fra senteret.
                            let near = pos.distance(r) < BuildingKind::Refinery.radius() + 2.0 * TILE;
                            if near {
                                let occupied = unloading_at.iter().any(|d| d.distance(r) < 1.0);
                                if !occupied {
                                    // Ledig dokk -> start lossing.
                                    self.units[i].harv = HarvState::Unloading;
                                    self.units[i].work_timer = UNLOAD_TIME;
                                    self.units[i].target = Some(r);
                                    self.units[i].path.clear();
                                    unloading_at.push(r);
                                    if team as u8 == TEAM_PLAYER {
                                        bridge::sound(SND_UNLOAD);
                                    }
                                } else {
                                    // Opptatt -> vent pa tur ved raffineriet.
                                    self.units[i].path.clear();
                                }
                            } else if self.units[i].path.is_empty() {
                                self.units[i].path = astar(&blocked, pos, r);
                                self.units[i].target = Some(r);
                            }
                        }
                        None => self.units[i].harv = HarvState::Idle,
                    }
                }
                HarvState::Unloading => {
                    // Loss gradvis: trekk fra lasten og tilfor kreditter litt etter
                    // litt, sa indikatoren synker jevnt i stedet for a hoppe til 0.
                    let t = self.units[i].work_timer.max(0.0001);
                    let dep = (self.units[i].carrying / t * dt).min(self.units[i].carrying);
                    self.units[i].carrying -= dep;
                    self.credits[team as usize] += dep;
                    self.units[i].work_timer -= dt;
                    if self.units[i].work_timer <= 0.0 {
                        // Resten inn, og frigjor dokken.
                        self.credits[team as usize] += self.units[i].carrying;
                        self.units[i].carrying = 0.0;
                        self.units[i].harv = HarvState::Idle;
                        self.units[i].target = None;
                    }
                }
            }
        }

        // Posisjoner for bevegelse -> brukes til a male fremgang (vranglas).
        let pre: Vec<Vec2> = self.units.iter().map(|u| u.pos).collect();

        // Bevegelse langs sti.
        for u in &mut self.units {
            // Ankomst-toleranse: er enheten naer det endelige malet, regn den som
            // framme (i en gruppe holder det a komme til naerheten av sitt felt --
            // da slutter den a presse mot et opptatt felt). Gjelder ikke hostere.
            if u.harv != HarvState::ToBase && u.harv != HarvState::ToOre {
                if let Some(t) = u.target {
                    if u.path.len() <= 1 && u.pos.distance(t) < unit_stats(u.kind).radius * 1.2 {
                        u.path.clear();
                    }
                }
            }
            while let Some(&wp) = u.path.first() {
                let to = wp - u.pos;
                let dist = to.length();
                if dist <= 3.0 {
                    u.path.remove(0);
                    continue;
                }
                let step = (unit_stats(u.kind).speed * dt).min(dist);
                u.pos += to / dist * step;
                break;
            }
            if u.path.is_empty() && u.target.is_some() && u.harv != HarvState::ToBase {
                u.target = None;
            }
        }

        // Separasjon med vikeplikt: den som har minst "rett" viker mest.
        // Prioritet: enheter i bevegelse > parkerte, og naer malet > langt unna.
        // Slik gir parkerte enheter plass, og i en passasje gar de pa rekke.
        let prio: Vec<f32> = self
            .units
            .iter()
            .map(|u| {
                if u.path.is_empty() {
                    0.0 // parkert -> viker for alle som er pa vei
                } else {
                    let rem = u.target.map_or(0.0, |t| u.pos.distance(t));
                    1.0 / (1.0 + rem * 0.01) // naer malet -> hoyere prioritet
                }
            })
            .collect();
        for i in 0..n {
            for j in (i + 1)..n {
                let d = self.units[j].pos - self.units[i].pos;
                let dist = d.length();
                let min_d = unit_stats(self.units[i].kind).radius + unit_stats(self.units[j].kind).radius;
                if dist <= 0.001 || dist >= min_d {
                    continue;
                }
                let dir = d / dist;
                let overlap = min_d - dist;
                let (pi, pj) = (prio[i], prio[j]);
                let (mut wi, mut wj) = if pi + pj > 0.0 {
                    (pj / (pi + pj), pi / (pi + pj))
                } else {
                    (0.5, 0.5)
                };
                // Ikke skyv noen inn i fjell/vann -> la den andre vike i stedet.
                if !self.passable_world(self.units[i].pos - dir * overlap * wi) {
                    wi = 0.0;
                    wj = 1.0;
                }
                if !self.passable_world(self.units[j].pos + dir * overlap * wj) {
                    wj = 0.0;
                    wi = 1.0;
                }
                let ni = self.units[i].pos - dir * overlap * wi;
                if self.passable_world(ni) {
                    self.units[i].pos = ni;
                }
                let nj = self.units[j].pos + dir * overlap * wj;
                if self.passable_world(nj) {
                    self.units[j].pos = nj;
                }
            }
        }

        // Fysisk kollisjon mot bygninger: enheter kan IKKE ga gjennom dem (sa
        // gjerder faktisk stopper fiender). Skyver ut langs minste inntrenging.
        let bcol: Vec<(Vec2, f32)> = self.buildings.iter().map(|b| (b.pos, b.kind.radius())).collect();
        for u in &mut self.units {
            let ur = unit_stats(u.kind).radius * 0.6;
            for &(bp, br) in &bcol {
                let half = br + ur;
                let dx = u.pos.x - bp.x;
                let dy = u.pos.y - bp.y;
                if dx.abs() < half && dy.abs() < half {
                    let px = half - dx.abs();
                    let py = half - dy.abs();
                    if px < py {
                        u.pos.x += if dx >= 0.0 { px } else { -px };
                    } else {
                        u.pos.y += if dy >= 0.0 { py } else { -py };
                    }
                }
            }
        }

        // Vranglas-bryting: enheter som har sti men ikke kommer seg fram, tar
        // et sidesteg (motsatt vei etter indeks, sa to som moter hverandre gar
        // hver sin vei). Henger det fortsatt -> regn ut ny sti.
        for i in 0..n {
            if self.units[i].path.is_empty() {
                self.units[i].stuck = 0.0;
                continue;
            }
            let moved = self.units[i].pos.distance(pre[i]);
            let want = unit_stats(self.units[i].kind).speed * dt * 0.35;
            if moved < want {
                self.units[i].stuck += dt;
            } else {
                self.units[i].stuck = 0.0;
            }
            if self.units[i].stuck > 0.4 {
                // Sidesteg mot den mest apne siden (sjekker fremkommelighet og
                // trengsel pa hver side), sa to som moter hverandre velger hver
                // sin vei i stedet for a presse mot samme kant.
                if let Some(&wp) = self.units[i].path.first() {
                    let pos = self.units[i].pos;
                    let to = wp - pos;
                    if to.length_squared() > 0.01 {
                        let dirn = to.normalize();
                        let perp = vec2(-dirn.y, dirn.x);
                        let probe = unit_stats(self.units[i].kind).radius * 2.2;
                        let score = |side: Vec2| -> f32 {
                            let test = pos + side * probe;
                            if !self.passable_world(test) {
                                return -1000.0;
                            }
                            // faerre naboer pa den siden = bedre
                            let crowd = self
                                .units
                                .iter()
                                .enumerate()
                                .filter(|(k, o)| *k != i && o.pos.distance(test) < probe)
                                .count() as f32;
                            -crowd
                        };
                        let (sl, sr) = (score(-perp), score(perp));
                        // ved likhet: bruk indeks for a bryte symmetri deterministisk
                        let side = if sr > sl || (sr == sl && i % 2 == 0) { perp } else { -perp };
                        let ns = pos + side * unit_stats(self.units[i].kind).speed * dt;
                        if self.passable_world(ns) {
                            self.units[i].pos = ns;
                        }
                    }
                }
            }
            if self.units[i].stuck > 1.0 {
                // Finn ny vei RUNDT stillestaende/fastlaste enheter.
                let pos = self.units[i].pos;
                if let Some(t) = self.units[i].target {
                    let ublocked = self.blocked_with_units(&blocked, i, t);
                    let np = astar(&ublocked, pos, t);
                    if !np.is_empty() {
                        self.units[i].path = np; // detour rundt folkemengden funnet
                    } else {
                        // Ingen vei rundt (f.eks. en bataljon sperrer en passasje).
                        // Rygg litt unna blokkeringen for a lage rom og bryte
                        // floken, sa prov a rute pa nytt -- gir "kjor tilbake og
                        // rundt"-oppforsel i stedet for a sta bom fast.
                        let r = unit_stats(self.units[i].kind).radius;
                        if let Some(&wp) = self.units[i].path.first() {
                            let back = (pos - wp).normalize_or_zero();
                            // prov a rygge rett bakover, ellers skratt til siden
                            for cand in [back, vec2(-back.y, back.x), vec2(back.y, -back.x)] {
                                let bp = pos + cand * r * 1.6;
                                if self.passable_world(bp) && !self.unit_overlaps_at(i, bp) {
                                    self.units[i].pos = bp;
                                    break;
                                }
                            }
                        }
                        self.units[i].path = astar(&blocked, self.units[i].pos, t);
                    }
                }
                self.units[i].stuck = 0.0;
            }
        }

        // Kamp.
        for u in &mut self.units {
            u.cooldown -= dt;
        }
        for b in &mut self.buildings {
            b.cooldown -= dt;
        }
        let usnap: Vec<(Vec2, u8, f32)> = self.units.iter().map(|u| (u.pos, u.team, u.hp)).collect();
        // (pos, lag, hp, radius, doemt) -- doemte bygninger kan beskytes av eget lag.
        let bsnap: Vec<(Vec2, u8, f32, f32, bool)> = self
            .buildings
            .iter()
            .map(|b| (b.pos, b.team, b.hp, b.kind.radius(), b.condemned))
            .collect();
        let mut udmg = vec![0.0f32; n];
        let mut bdmg = vec![0.0f32; self.buildings.len()];
        for i in 0..n {
            let s = unit_stats(self.units[i].kind);
            if s.damage <= 0.0 || self.units[i].cooldown > 0.0 {
                continue;
            }
            let (pos, team, _) = usnap[i];
            let mut best: Option<(bool, usize)> = None;
            let mut bd = s.range;
            for (j, &(p, t, hp)) in usnap.iter().enumerate() {
                if t == team || hp <= 0.0 {
                    continue;
                }
                let d = pos.distance(p);
                if d < bd {
                    bd = d;
                    best = Some((false, j));
                }
            }
            for (j, &(p, t, hp, r, condemned)) in bsnap.iter().enumerate() {
                // Fiendebygg, eller egne DOEMTE bygg (riving).
                if hp <= 0.0 || (t == team && !condemned) {
                    continue;
                }
                let d = (pos.distance(p) - r).max(0.0);
                if d < bd {
                    bd = d;
                    best = Some((true, j));
                }
            }
            if let Some((is_bld, j)) = best {
                self.units[i].cooldown = s.fire;
                let to = if is_bld { bsnap[j].0 } else { usnap[j].0 };
                if team == TEAM_PLAYER {
                    bridge::sound(SND_SHOOT);
                }
                self.shots.push(Shot {
                    from: pos,
                    to,
                    team,
                    life: SHOT_LIFETIME,
                });
                // Fiende-enheter skalerer skade med nivaets enemy_power.
                let dmg = if team == TEAM_ENEMY { s.damage * self.enemy_power } else { s.damage };
                if is_bld {
                    bdmg[j] += dmg;
                } else {
                    udmg[j] += dmg;
                }
            }
        }
        // Vakttarn fyrer en varmesokende storkule mot naermeste fiende.
        for bi in 0..self.buildings.len() {
            let (dmg, range, fire, _sight) = match self.buildings[bi].kind.combat() {
                Some(c) => c,
                None => continue,
            };
            if self.buildings[bi].condemned || !self.buildings[bi].operational() || self.buildings[bi].cooldown > 0.0 {
                continue;
            }
            let bpos = self.buildings[bi].pos;
            let bteam = self.buildings[bi].team;
            let mut best: Option<Vec2> = None;
            let mut bd = range;
            for &(p, t, hp) in usnap.iter() {
                if t == bteam || hp <= 0.0 {
                    continue;
                }
                let d = bpos.distance(p);
                if d < bd {
                    bd = d;
                    best = Some(p);
                }
            }
            if let Some(tp) = best {
                self.buildings[bi].cooldown = fire;
                let dir = (tp - bpos).normalize_or_zero();
                self.projectiles.push(Projectile { pos: bpos, dir, team: bteam, damage: dmg, life: 3.0 });
                if bteam == TEAM_PLAYER {
                    bridge::sound(SND_TURRET);
                }
            }
        }
        for (i, d) in udmg.iter().enumerate() {
            if *d > 0.0 && !(self.god_mode && self.units[i].team == TEAM_PLAYER) {
                self.units[i].hp -= d;
            }
        }
        for (i, d) in bdmg.iter().enumerate() {
            if *d > 0.0 && !(self.god_mode && self.buildings[i].team == TEAM_PLAYER) {
                self.buildings[i].hp -= d;
            }
        }

        // Homing-prosjektiler: svinger mot naermeste fiende og treffer.
        {
            let usnap2: Vec<(Vec2, u8, f32)> = self.units.iter().map(|u| (u.pos, u.team, u.hp)).collect();
            let mut hit = vec![0.0f32; self.units.len()];
            for pr in &mut self.projectiles {
                let mut tgt: Option<Vec2> = None;
                let mut bd = f32::MAX;
                for (j, &(q, t, hp)) in usnap2.iter().enumerate() {
                    if t == pr.team || hp <= 0.0 {
                        continue;
                    }
                    let d = pr.pos.distance(q);
                    if d < bd {
                        bd = d;
                        tgt = Some(q);
                        if d < PROJECTILE_HIT {
                            hit[j] += pr.damage;
                            pr.life = 0.0;
                        }
                    }
                }
                if let Some(q) = tgt {
                    let desired = (q - pr.pos).normalize_or_zero();
                    // Skarpere sving jo naermere malet, sa den buer rett inn i
                    // stedet for a sirkle rundt det. Faktoren klemmes til 1.0 slik
                    // at den kan snappe direkte mot malet pa kloss hold.
                    let turn = if bd < 110.0 { PROJECTILE_TURN * 3.0 } else { PROJECTILE_TURN };
                    let factor = (turn * dt).min(1.0);
                    pr.dir = (pr.dir + (desired - pr.dir) * factor).normalize_or_zero();
                    // Brems litt nar den er naer, sa svingeradien blir mindre.
                    let speed = if bd < 70.0 { PROJECTILE_SPEED * 0.6 } else { PROJECTILE_SPEED };
                    pr.pos += pr.dir * speed * dt;
                } else {
                    pr.pos += pr.dir * PROJECTILE_SPEED * dt;
                }
                pr.life -= dt;
            }
            self.projectiles.retain(|p| p.life > 0.0);
            for (j, d) in hit.iter().enumerate() {
                if *d > 0.0 && !(self.god_mode && self.units[j].team == TEAM_PLAYER) {
                    self.units[j].hp -= d;
                }
            }
        }

        let before = self.units.len() + self.buildings.len();
        self.units.retain(|u| u.hp > 0.0);
        self.buildings.retain(|b| b.hp > 0.0);
        if self.units.len() + self.buildings.len() < before {
            bridge::sound(SND_EXPLOSION);
        }

        // Ferdigstill bygg som ventet pa at en gammel skulle rives.
        let nb = self.buildings.len();
        for i in 0..nb {
            if let Some(oldpos) = self.buildings[i].awaiting {
                let old_alive = (0..nb).any(|j| j != i && self.buildings[j].pos.distance(oldpos) < 1.0);
                if !old_alive {
                    self.buildings[i].awaiting = None; // na operativ
                }
            }
        }

        for s in &mut self.shots {
            s.life -= dt;
        }
        self.shots.retain(|s| s.life > 0.0);

        let p_hq = self.buildings.iter().any(|b| b.team == TEAM_PLAYER && b.kind == BuildingKind::Hq);
        let e_hq = self.buildings.iter().any(|b| b.team == TEAM_ENEMY && b.kind == BuildingKind::Hq);
        if !p_hq {
            self.outcome = Some(false);
            bridge::sound(SND_LOSE);
        } else if !e_hq {
            self.outcome = Some(true);
            bridge::sound(SND_WIN);
            self.unlock_next();
        }
    }

    // Las opp neste niva (etter seier) sa det blir valgbart i nivamenyen.
    fn unlock_next(&mut self) {
        let next = (self.level + 1).min(levels::count().saturating_sub(1));
        if next > self.max_unlocked {
            self.max_unlocked = next;
        }
    }

    // ----- kommandoer fra nettleseren -----

    fn apply_command(&mut self, code: i32, a: [f32; 4]) {
        let kind_of = |k: f32| match k as i32 {
            1 => UnitKind::Tank,
            2 => UnitKind::Harvester,
            _ => UnitKind::Rifleman,
        };
        let bkind_of = |k: f32| match k as i32 {
            1 => BuildingKind::Factory,
            2 => BuildingKind::Wall,
            3 => BuildingKind::Turret,
            _ => BuildingKind::Refinery,
        };
        match code {
            1 => {
                let lvl = self.level; // spill gjeldende niva pa nytt
                self.load_level(lvl);
            }
            2 => {
                // Sentrer pa spillerens base (HK), ikke kartmidten (som er
                // uutforsket/svart). Fall tilbake til enhet, sa kartmidte.
                let focus = self
                    .buildings
                    .iter()
                    .find(|b| b.team == TEAM_PLAYER && b.kind == BuildingKind::Hq)
                    .map(|b| b.pos)
                    .or_else(|| self.units.iter().find(|u| u.team == TEAM_PLAYER).map(|u| u.pos))
                    .unwrap_or_else(|| vec2(MAP_W as f32 * TILE, MAP_H as f32 * TILE) * 0.5);
                let view = vec2(screen_width(), screen_height()) / self.zoom;
                self.cam = focus - view * 0.5;
                self.clamp_camera();
            }
            3 => self.cam = vec2(a[0], a[1]),
            4 => {
                // Zoom om SENTERET av spillomradet, sa man zoomer inn der man
                // ser (ikke mot kartets hjorne).
                let center = vec2(self.play_w() * 0.5, screen_height() * 0.5);
                let before = self.screen_to_world(center);
                self.zoom = a[0].clamp(0.4, 3.0);
                let after = self.screen_to_world(center);
                self.cam += before - after;
            }
            5 => {
                let team = if a[0] as i32 == 1 { TEAM_ENEMY } else { TEAM_PLAYER };
                self.units.push(Unit::new(vec2(a[2], a[3]), team, kind_of(a[1])));
                self.outcome = None;
            }
            6 => self.build(TEAM_PLAYER, kind_of(a[0])),
            7 => {
                let team = (a[0] as i32).clamp(0, 1) as usize;
                self.credits[team] += a[1];
            }
            8 => {
                let on = a[1] != 0.0;
                match a[0] as i32 {
                    0 => self.paused = on,
                    1 => self.free_build = on,
                    2 => self.god_mode = on,
                    3 => self.reveal = on,
                    _ => {}
                }
            }
            9 => self.speed = a[0].clamp(0.1, 8.0),
            10 => {
                self.rally[TEAM_PLAYER as usize] = vec2(a[0], a[1]);
                self.rally_show = 1.0;
            }
            11 => {
                // Flytt alle spillerens kampenheter til (a[0], a[1]) via pathfinding.
                let dest = vec2(a[0], a[1]);
                let blocked = self.compute_blocked();
                for u in &mut self.units {
                    if u.team == TEAM_PLAYER && u.kind != UnitKind::Harvester {
                        u.path = astar(&blocked, u.pos, dest);
                        u.target = Some(dest);
                    }
                }
            }
            12 => self.mouse_active = false, // pekeren forlot vinduet -> stopp kant-scroll
            13 => {
                // Start plassering av bygning (0=RAF, 1=FAB, 2=Gjerde, 3=Tarn).
                self.placing = Some(bkind_of(a[0]));
                self.move_src = None;
            }
            14 => {
                // Plasser bygning direkte pa (a1,a2).
                self.place_building(bkind_of(a[0]), vec2(a[1], a[2]));
            }
            15 => {
                // Velg sprak ut fra flaggvelger-indeks.
                self.lang = i18n::from_index(a[0] as usize);
            }
            16 => {
                // Burger: vis/skjul byggmenyen.
                self.sidebar_open = a[0] != 0.0;
            }
            17 => {
                // Joystick: sett kamera-pan-hastighet (klemt til lengde 1).
                let v = vec2(a[0], a[1]);
                self.pan_vel = if v.length() > 1.0 { v.normalize() } else { v };
            }
            _ => {}
        }
    }

    // ----- tegning -----

    fn draw(&self) {
        clear_background(Color::new(0.05, 0.06, 0.07, 1.0));

        // Start/Guide-skjerm: tegn menyen i stedet for selve spillet.
        if self.screen != Screen::Playing {
            self.draw_menu();
            return;
        }

        let tl = self.screen_to_world(vec2(0.0, 0.0));
        let br = self.screen_to_world(vec2(self.play_w(), screen_height()));
        let x0 = (tl.x / TILE).floor().max(0.0) as usize;
        let y0 = (tl.y / TILE).floor().max(0.0) as usize;
        let x1 = ((br.x / TILE).ceil() as usize).min(MAP_W);
        let y1 = ((br.y / TILE).ceil() as usize).min(MAP_H);
        let fog = Color::new(0.0, 0.0, 0.0, 0.5);
        for ty in y0..y1 {
            for tx in x0..x1 {
                let idx = ty * MAP_W + tx;
                if !self.explored[idx] {
                    continue; // uutforsket -> svart (bakgrunn)
                }
                let p = self.world_to_screen(vec2(tx as f32 * TILE, ty as f32 * TILE));
                self.draw_terrain_tile(tx, ty, p);
                if !self.visible[idx] {
                    let s = TILE * self.zoom + 1.0;
                    draw_rectangle(p.x, p.y, s, s, fog); // utforsket, men ikke synlig
                }
            }
        }

        let factory_sel = self.buildings.iter().any(|b| b.selected && b.kind == BuildingKind::Factory);
        if factory_sel || self.rally_show > 0.0 {
            // Samlepunkt: et flagg der nye enheter moter opp. Vises mens fabrikken
            // er valgt, og en kort stund (~1 s) etter at det ble satt selv om
            // byggmenyen/markeringen forsvant.
            let r = self.world_to_screen(self.rally[TEAM_PLAYER as usize]);
            let fade = if factory_sel { 0.95 } else { (self.rally_show / 1.0).clamp(0.0, 1.0) * 0.95 };
            let g = Color::new(0.25, 0.9, 0.3, fade);
            draw_line(r.x, r.y, r.x, r.y - 22.0, 2.0, g); // stang
            draw_triangle(vec2(r.x, r.y - 22.0), vec2(r.x + 16.0, r.y - 17.0), vec2(r.x, r.y - 12.0), g); // flagg
            draw_circle(r.x, r.y, 3.0, g); // fot
            draw_circle_lines(r.x, r.y, 9.0, 1.5, Color::new(0.25, 0.9, 0.3, 0.5));
            txt(self.t(Key::RallyPoint), r.x + 6.0, r.y + 16.0, 15.0, g);
        }

        // "Kjor hit"-merke der enheter ble sendt: grønt kryss + pulserende ring.
        if let Some((p, t)) = self.move_marker {
            let s = self.world_to_screen(p);
            let a = (t / 0.8).clamp(0.0, 1.0);
            let col = Color::new(0.30, 0.95, 0.40, a);
            draw_circle_lines(s.x, s.y, 4.0 + (1.0 - a) * 14.0, 2.0, col); // ring vokser utover mens den toner ut
            let c = 7.0;
            draw_line(s.x - c, s.y, s.x + c, s.y, 2.0, col);
            draw_line(s.x, s.y - c, s.x, s.y + c, 2.0, col);
        }

        // Bygninger (fiendens kun nar synlig).
        for b in &self.buildings {
            if b.team != TEAM_PLAYER && !self.tile_visible(b.pos) {
                continue;
            }
            self.draw_building(b);
        }

        for s in &self.shots {
            if !self.tile_visible(s.from) && !self.tile_visible(s.to) {
                continue;
            }
            let a = self.world_to_screen(s.from);
            let b = self.world_to_screen(s.to);
            let col = if s.team == TEAM_PLAYER { YELLOW } else { ORANGE };
            draw_line(a.x, a.y, b.x, b.y, 2.0, col);
            draw_circle(b.x, b.y, 2.5, col);
        }

        // Storkuler (tarn): glodende kule med hale.
        for pr in &self.projectiles {
            if !self.tile_visible(pr.pos) {
                continue;
            }
            let sp = self.world_to_screen(pr.pos);
            let z = self.zoom.max(0.6);
            let tail = sp - pr.dir * 12.0 * z;
            draw_line(tail.x, tail.y, sp.x, sp.y, 3.0 * z, Color::new(1.0, 0.55, 0.1, 0.7));
            draw_circle(sp.x, sp.y, 6.0 * z, Color::new(1.0, 0.5, 0.1, 0.5)); // glod
            draw_circle(sp.x, sp.y, 3.5 * z, Color::new(1.0, 0.85, 0.3, 1.0)); // kjerne
        }

        // Enheter (fiendens kun nar synlig).
        for u in &self.units {
            if u.team != TEAM_PLAYER && !self.tile_visible(u.pos) {
                continue;
            }
            self.draw_unit(u);
        }

        if let Some(start) = self.drag_start {
            // start er verdens-koordinat -> tegn fra dens skjermposisjon (flytter
            // seg nar kameraet panorerer, sa boksen folger kartpunktet).
            let s = self.world_to_screen(start);
            let (mx, my) = mouse_position();
            let min = s.min(vec2(mx, my));
            let max = s.max(vec2(mx, my));
            let size = max - min;
            if size.length() > 6.0 {
                draw_rectangle(min.x, min.y, size.x, size.y, Color::new(0.2, 0.9, 0.2, 0.10));
                draw_rectangle_lines(min.x, min.y, size.x, size.y, 1.5, GREEN);
            }
        }

        // Spokelse for bygning under plassering: gronn = gyldig, rod = ugyldig.
        if let Some(kind) = self.placing {
            let (mx, my) = mouse_position();
            if !self.in_sidebar(vec2(mx, my)) {
                let w = self.screen_to_world(vec2(mx, my));
                let rr = kind.radius() * self.zoom;
                // Gjerde-rekke: vis hele linja mens man drar.
                let centers = match self.wall_drag {
                    Some(start) if kind == BuildingKind::Wall => self.wall_line_tiles(start, w),
                    _ => vec![Self::snap_to_tile(w)],
                };
                for center in &centers {
                    let ok = self.can_place_building(kind, *center)
                        && (self.free_build || self.credits[0] >= kind.cost());
                    let sp = self.world_to_screen(*center);
                    let fill = if ok { Color::new(0.2, 0.9, 0.3, 0.30) } else { Color::new(0.9, 0.2, 0.2, 0.30) };
                    draw_rectangle(sp.x - rr, sp.y - rr, rr * 2.0, rr * 2.0, fill);
                    draw_rectangle_lines(sp.x - rr, sp.y - rr, rr * 2.0, rr * 2.0, 2.0, if ok { GREEN } else { RED });
                }
                // Etikett ved markoren.
                let sp = self.world_to_screen(*centers.last().unwrap());
                let tag = if self.move_src.is_some() {
                    self.t(Key::ActionMove)
                } else if kind == BuildingKind::Wall && centers.len() > 1 {
                    self.t(Key::BldWall)
                } else {
                    self.t(kind.label_key())
                };
                txt(tag, sp.x - rr + 4.0, sp.y + 5.0, 16.0, WHITE);
            }
        }

        self.draw_sidebar();
        self.draw_hud();
        self.draw_controls();
    }

    // Terrengrute med litt liv: gress-tuster, gull-glitter pa malm (mindre nar
    // det tappes), boelger pa vann, steinblokker pa fjell.
    fn draw_terrain_tile(&self, tx: usize, ty: usize, p: Vec2) {
        let idx = ty * MAP_W + tx;
        let t = self.map[idx];
        let z = self.zoom;
        let s = TILE * z + 1.0;
        draw_rectangle(p.x, p.y, s, s, t.color(tx, ty));
        let h = hash2(tx as i32 * 7 + 1, ty as i32 * 13 + 3);
        match t {
            Terrain::Grass => {
                if h > 0.78 {
                    let tuft = Color::new(0.16, 0.30, 0.12, 1.0);
                    let gx = p.x + h * TILE * 0.6 * z;
                    let gy = p.y + (1.0 - h) * TILE * 0.6 * z;
                    draw_rectangle(gx, gy, 2.0 * z.max(0.6), 4.0 * z.max(0.6), tuft);
                }
            }
            Terrain::Ore => {
                // Mengde glitter folger gjenvaerende malm.
                let frac = (self.ore[idx] / ORE_PER_TILE).clamp(0.0, 1.0);
                let dots = (frac * 5.0).round() as i32;
                for k in 0..dots {
                    let hk = hash2(tx as i32 * 31 + k * 17, ty as i32 * 19 + k * 7);
                    let hk2 = hash2(tx as i32 * 13 + k * 5, ty as i32 * 29 + k * 11);
                    let gx = p.x + hk * (TILE - 4.0) * z;
                    let gy = p.y + hk2 * (TILE - 4.0) * z;
                    let bright = Color::new(0.95, 0.85, 0.35, 1.0);
                    draw_rectangle(gx, gy, 2.2 * z.max(0.6), 2.2 * z.max(0.6), bright);
                }
            }
            Terrain::Water => {
                let wave = Color::new(0.30, 0.48, 0.66, 0.5);
                let wy = p.y + (0.3 + 0.4 * h) * s;
                draw_line(p.x + 3.0, wy, p.x + s - 3.0, wy, 1.5, wave);
            }
            Terrain::Rock => {
                let chunk = Color::new(0.22, 0.22, 0.24, 1.0);
                let gx = p.x + h * TILE * 0.4 * z;
                let gy = p.y + (1.0 - h) * TILE * 0.4 * z;
                draw_rectangle(gx, gy, TILE * 0.4 * z, TILE * 0.35 * z, chunk);
            }
        }
    }

    fn draw_building(&self, b: &Building) {
        let p = self.world_to_screen(b.pos);
        let r = b.kind.radius() * self.zoom;
        let z = self.zoom;
        let trim = team_color(b.team);
        let steel = Color::new(0.24, 0.26, 0.30, 1.0);
        let dark = Color::new(0.13, 0.14, 0.16, 1.0);
        // skygge
        draw_rectangle(p.x - r + 3.0 * z, p.y - r + 3.0 * z, r * 2.0, r * 2.0, Color::new(0.0, 0.0, 0.0, 0.25));
        // hovedkropp
        draw_rectangle(p.x - r, p.y - r, r * 2.0, r * 2.0, steel);
        let light = Color::new(0.34, 0.37, 0.42, 1.0); // lysere stal (kant/høylys)
        let glass = Color::new(0.45, 0.72, 0.85, 1.0);
        let ore_col = Color::new(0.62, 0.52, 0.20, 1.0);
        let time = get_time();
        match b.kind {
            BuildingKind::Hq => {
                // Kommandosenter: tak-trim, hevet midtblokk, vindusrekke,
                // roterende radar og blinkende fyr.
                draw_rectangle(p.x - r, p.y - r, r * 2.0, r * 0.5, trim); // tak
                draw_rectangle(p.x - r, p.y - r * 0.5, r * 2.0, 2.0, dark);
                // hevet midtblokk
                draw_rectangle(p.x - r * 0.55, p.y - r * 0.95, r * 1.1, r * 0.45, light);
                draw_rectangle_lines(p.x - r * 0.55, p.y - r * 0.95, r * 1.1, r * 0.45, 1.5, dark);
                // vindusrekke nederst
                for k in 0..3 {
                    let wx = p.x - r * 0.7 + k as f32 * r * 0.6;
                    draw_rectangle(wx, p.y + r * 0.45, r * 0.32, r * 0.32, glass);
                    draw_rectangle_lines(wx, p.y + r * 0.45, r * 0.32, r * 0.32, 1.0, dark);
                }
                // radar med sveipende arm
                let (cx, cy) = (p.x + r * 0.4, p.y + r * 0.05);
                draw_circle(cx, cy, r * 0.42, dark);
                draw_circle_lines(cx, cy, r * 0.42, 2.0, light);
                let a = (time * 1.4) as f32;
                draw_line(cx, cy, cx + a.cos() * r * 0.42, cy + a.sin() * r * 0.42, 2.0, trim);
                // blinkende fyr pa toppen
                let blink = if (time * 1.6).sin() > 0.0 { Color::new(1.0, 0.25, 0.2, 1.0) } else { Color::new(0.4, 0.1, 0.1, 1.0) };
                draw_circle(p.x, p.y - r * 0.95, r * 0.12, blink);
            }
            BuildingKind::Refinery => {
                // Raffineri: to siloer med band, ror, og dokk-pad med farestriper.
                draw_rectangle(p.x - r, p.y - r, r * 2.0, r * 0.35, trim); // tak-trim
                // stor silo
                let (sx, sy, sr) = (p.x - r * 0.45, p.y + r * 0.05, r * 0.5);
                draw_circle(sx, sy, sr, ore_col);
                draw_circle_lines(sx, sy, sr, 2.0, dark);
                draw_line(sx - sr, sy - sr * 0.35, sx + sr, sy - sr * 0.35, 1.5, Color::new(0.40, 0.33, 0.14, 1.0));
                draw_line(sx - sr, sy + sr * 0.35, sx + sr, sy + sr * 0.35, 1.5, Color::new(0.40, 0.33, 0.14, 1.0));
                // liten silo
                draw_circle(p.x + r * 0.05, p.y - r * 0.25, r * 0.28, ore_col);
                draw_circle_lines(p.x + r * 0.05, p.y - r * 0.25, r * 0.28, 1.5, dark);
                // ror mellom siloene
                draw_line(sx, sy - sr * 0.6, p.x + r * 0.05, p.y - r * 0.25, 3.0, light);
                // dokk-pad med farestriper
                let (dx, dy, dw, dh) = (p.x + r * 0.1, p.y + r * 0.25, r * 0.85, r * 0.65);
                draw_rectangle(dx, dy, dw, dh, dark);
                for k in 0..3 {
                    let lx = dx + k as f32 * (dw / 3.0);
                    draw_triangle(vec2(lx, dy), vec2(lx + dw / 6.0, dy), vec2(lx, dy + dh * 0.5), Color::new(0.85, 0.7, 0.2, 0.8));
                }
            }
            BuildingKind::Factory => {
                // Fabrikk: tak-trim, garasjeport med slisser, fareband, gantry
                // og pipe med rok.
                draw_rectangle(p.x - r, p.y - r, r * 2.0, r * 0.4, trim); // tak
                // gantry-bjelke pa taket
                draw_rectangle(p.x - r * 0.9, p.y - r * 0.95, r * 1.8, r * 0.12, light);
                // fareband over porten
                let (dx, dy, dw, dh) = (p.x - r * 0.62, p.y - r * 0.02, r * 1.24, r * 0.95);
                for k in 0..6 {
                    let hx = dx + k as f32 * (dw / 6.0);
                    let col = if k % 2 == 0 { Color::new(0.85, 0.7, 0.2, 1.0) } else { dark };
                    draw_rectangle(hx, dy - r * 0.12, dw / 6.0, r * 0.1, col);
                }
                // garasjeport med horisontale slisser
                draw_rectangle(dx, dy, dw, dh, dark);
                for k in 0..5 {
                    let ly = dy + (k as f32 + 0.5) * (dh / 5.0);
                    draw_line(dx, ly, dx + dw, ly, 1.0, Color::new(0.30, 0.30, 0.33, 1.0));
                }
                // pipe med stigende rok
                let stx = p.x + r * 0.72;
                draw_rectangle(stx - r * 0.1, p.y - r * 0.9, r * 0.2, r * 0.5, light);
                for k in 0..3 {
                    let t = ((time * 0.6) as f32 + k as f32 * 0.33) % 1.0;
                    let sy = p.y - r * 0.9 - t * r * 0.9;
                    draw_circle(stx + (t * 6.0).sin() * r * 0.12, sy, r * 0.14 * (1.0 - t) + 1.0, Color::new(0.6, 0.6, 0.6, 0.4 * (1.0 - t)));
                }
            }
            BuildingKind::Wall => {
                // Gjerde: stolper + tverrlist, lag-farget topp.
                draw_rectangle(p.x - r, p.y - r * 0.4, r * 2.0, r * 0.8, Color::new(0.30, 0.27, 0.22, 1.0));
                draw_rectangle(p.x - r, p.y - r * 0.4, r * 2.0, r * 0.25, trim);
                for k in 0..3 {
                    let px = p.x - r + (k as f32 + 0.5) * (r * 2.0 / 3.0);
                    draw_rectangle(px - 1.5, p.y - r, 3.0, r * 2.0, dark);
                }
            }
            BuildingKind::Turret => {
                // Vakttarn: sokkel + roterende kanon (scanner nar ingen fiende).
                draw_circle(p.x, p.y, r * 0.95, Color::new(0.22, 0.24, 0.28, 1.0));
                draw_circle_lines(p.x, p.y, r * 0.95, 2.0, dark);
                draw_circle(p.x, p.y, r * 0.5, trim);
                let a = (time * 0.8) as f32;
                draw_line(p.x, p.y, p.x + a.cos() * r * 1.5, p.y + a.sin() * r * 1.5, 4.0, dark);
            }
        }
        // Bygningens ramme tegnes ikke for runde tarn.
        if b.kind != BuildingKind::Turret {
            draw_rectangle_lines(p.x - r, p.y - r, r * 2.0, r * 2.0, 2.0, dark);
        }
        if b.selected {
            draw_rectangle_lines(p.x - r - 3.0, p.y - r - 3.0, r * 2.0 + 6.0, r * 2.0 + 6.0, 2.0, GREEN);
        }
        // Doemt til riving -> blinkende rod ramme + merke.
        if b.condemned && (time * 4.0).sin() > 0.0 {
            draw_rectangle_lines(p.x - r - 2.0, p.y - r - 2.0, r * 2.0 + 4.0, r * 2.0 + 4.0, 3.0, RED);
        }
        // Under bygging (venter pa at gammel rives) -> dimmes + stillas + merke.
        if !b.operational() {
            draw_rectangle(p.x - r, p.y - r, r * 2.0, r * 2.0, Color::new(0.0, 0.0, 0.0, 0.45));
            for k in 0..3 {
                let ly = p.y - r + (k as f32 + 1.0) * (r * 2.0 / 4.0);
                draw_line(p.x - r, ly, p.x + r, ly, 1.0, Color::new(0.85, 0.7, 0.2, 0.5));
            }
            txt(self.t(Key::Constructing), p.x - r + 2.0, p.y, 14.0, Color::new(1.0, 0.9, 0.4, 1.0));
        }
        if b.kind != BuildingKind::Wall {
            txt(self.t(b.kind.label_key()), p.x - r + 4.0, p.y + r - 5.0, 16.0, Color::new(1.0, 1.0, 1.0, 0.85));
        }
        let frac = (b.hp / b.kind.max_hp()).clamp(0.0, 1.0);
        draw_rectangle(p.x - r, p.y - r - 7.0, r * 2.0, 4.0, Color::new(0.1, 0.1, 0.1, 0.85));
        draw_rectangle(p.x - r, p.y - r - 7.0, r * 2.0 * frac, 4.0, if frac > 0.5 { GREEN } else if frac > 0.25 { YELLOW } else { RED });
    }

    fn draw_unit(&self, u: &Unit) {
        let p = self.world_to_screen(u.pos);
        let st = unit_stats(u.kind);
        let r = st.radius * self.zoom;
        let trim = team_color(u.team);
        let dark = Color::new(0.11, 0.11, 0.13, 1.0);
        // retning fra neste veipunkt (faller tilbake til "ned")
        let dir = u.path.first().map(|w| *w - u.pos).unwrap_or(Vec2::ZERO);
        let f = if dir.length_squared() > 1.0 { dir.normalize() } else { vec2(0.0, 1.0) };
        let ang = f.y.atan2(f.x);
        if u.selected {
            draw_circle_lines(p.x, p.y, r + 5.0, 2.0, GREEN);
        }
        draw_circle(p.x + 2.0, p.y + 2.0, r, Color::new(0.0, 0.0, 0.0, 0.2)); // skygge
        match u.kind {
            UnitKind::Rifleman => {
                draw_circle(p.x, p.y, r, trim);
                draw_circle(p.x, p.y, r * 0.55, dark); // hjelm
                draw_line(p.x, p.y, p.x + f.x * r * 1.4, p.y + f.y * r * 1.4, 2.0, dark); // rifle
            }
            UnitKind::Tank => {
                let rot = |w: f32, h: f32, col: Color| {
                    draw_rectangle_ex(p.x, p.y, w, h, DrawRectangleParams { offset: vec2(0.5, 0.5), rotation: ang, color: col });
                };
                rot(r * 2.0, r * 1.8, dark); // belter
                rot(r * 1.9, r * 1.25, trim); // skrog
                draw_circle(p.x, p.y, r * 0.62, Color::new(0.16, 0.17, 0.20, 1.0)); // taarn
                draw_line(p.x, p.y, p.x + f.x * r * 1.7, p.y + f.y * r * 1.7, 3.0, dark); // kanon
            }
            UnitKind::Harvester => {
                let body = if u.team == TEAM_PLAYER {
                    Color::new(0.78, 0.66, 0.22, 1.0)
                } else {
                    Color::new(0.74, 0.50, 0.20, 1.0)
                };
                draw_rectangle_ex(p.x, p.y, r * 2.3, r * 1.8, DrawRectangleParams { offset: vec2(0.5, 0.5), rotation: ang, color: dark });
                draw_rectangle_ex(p.x, p.y, r * 2.0, r * 1.5, DrawRectangleParams { offset: vec2(0.5, 0.5), rotation: ang, color: body });
                // skuffe foran
                draw_rectangle_ex(p.x + f.x * r * 1.0, p.y + f.y * r * 1.0, r * 0.7, r * 1.7, DrawRectangleParams { offset: vec2(0.5, 0.5), rotation: ang, color: dark });
                draw_rectangle_ex(p.x, p.y, r * 0.6, r * 0.6, DrawRectangleParams { offset: vec2(0.5, 0.5), rotation: ang, color: trim }); // lag-merke
                let fill = (u.carrying / HARVEST_CAPACITY).clamp(0.0, 1.0);
                if fill > 0.0 {
                    draw_circle(p.x, p.y, r * 0.35 * fill, Color::new(0.95, 0.85, 0.35, 1.0));
                }
                // Last-stolpe: fylles ved graving, SYNKER ved lossing.
                let cbw = r * 2.0;
                draw_rectangle(p.x - r, p.y - r - 13.0, cbw, 3.0, Color::new(0.1, 0.1, 0.1, 0.8));
                draw_rectangle(p.x - r, p.y - r - 13.0, cbw * fill, 3.0, Color::new(0.95, 0.82, 0.30, 1.0));
                // Lossing: animert nedoverpil som viser at lasten tommes.
                if u.harv == HarvState::Unloading {
                    let t = (get_time() * 4.0) as f32;
                    let off = (t.sin() * 0.5 + 0.5) * r * 0.7;
                    let ay = p.y - r - 20.0 + off;
                    draw_triangle(
                        vec2(p.x - 4.0, ay),
                        vec2(p.x + 4.0, ay),
                        vec2(p.x, ay + 6.0),
                        Color::new(0.95, 0.82, 0.30, 0.9),
                    );
                }
            }
        }
        let frac = (u.hp / u.max_hp).clamp(0.0, 1.0);
        let bw = r * 2.0;
        draw_rectangle(p.x - r, p.y - r - 8.0, bw, 3.0, Color::new(0.1, 0.1, 0.1, 0.8));
        let hp_col = if frac > 0.5 { GREEN } else if frac > 0.25 { YELLOW } else { RED };
        draw_rectangle(p.x - r, p.y - r - 8.0, bw * frac, 3.0, hp_col);
    }

    fn draw_sidebar(&self) {
        if !self.sidebar_on() {
            // Byggmenyen er skjult (burger) -> vis i det minste produksjons-
            // progress oppe til hoyre sa man ser hva som bygges.
            let p = &self.prod[TEAM_PLAYER as usize];
            if !p.active.is_empty() || !p.queue.is_empty() {
                let w = 156.0;
                let x = screen_width() - w - 66.0; // til venstre for burger-knappen
                let mut y = 36.0;
                for (k, rem) in &p.active {
                    let frac = (1.0 - rem / unit_stats(*k).build_time).clamp(0.0, 1.0);
                    draw_rectangle(x, y, w, 18.0, Color::new(0.10, 0.12, 0.14, 0.85));
                    draw_rectangle(x, y, w * frac, 18.0, Color::new(0.25, 0.6, 0.35, 0.9));
                    draw_rectangle_lines(x, y, w, 18.0, 1.5, Color::new(0.4, 0.5, 0.45, 0.9));
                    txt(self.t(k.name_key()), x + 5.0, y + 14.0, 13.0, WHITE);
                    y += 22.0;
                }
                let q = p.queue.len();
                if q > 0 {
                    txt(&format!("+{}", q), x + w - 30.0, y + 12.0, 14.0, Color::new(0.6, 0.9, 1.0, 1.0));
                }
            }
            return;
        }
        let x = self.play_w();
        let w = SIDEBAR_W;
        draw_rectangle(x, 0.0, w, screen_height(), Color::new(0.08, 0.09, 0.10, 1.0));
        draw_rectangle(x, 0.0, 2.0, screen_height(), team_color(TEAM_PLAYER));
        txt(self.t(Key::BuildHeader), x + 10.0, 24.0, 20.0, Color::new(0.85, 0.92, 1.0, 1.0));

        // Penge-oversikt: kreditter + antall hostere (inntektskilde).
        let bx = x + 8.0;
        let bw = w - 16.0;
        draw_rectangle(bx, 32.0, bw, 24.0, Color::new(0.14, 0.16, 0.10, 1.0));
        draw_rectangle_lines(bx, 32.0, bw, 24.0, 1.5, Color::new(0.40, 0.36, 0.12, 1.0));
        draw_circle(bx + 13.0, 44.0, 6.5, Color::new(0.92, 0.80, 0.25, 1.0));
        draw_circle_lines(bx + 13.0, 44.0, 6.5, 1.5, Color::new(0.55, 0.45, 0.10, 1.0));
        let harv = self.count_units(TEAM_PLAYER, UnitKind::Harvester);
        txt(&format!("{}", self.credits[0] as i32), bx + 25.0, 49.0, 20.0, Color::new(0.98, 0.92, 0.55, 1.0));
        txt(&format!("{}{}", harv, self.t(Key::HarvShort)), bx + bw - 24.0, 49.0, 17.0, Color::new(0.75, 0.85, 0.7, 1.0));

        self.draw_minimap();

        let mp = {
            let (mx, my) = mouse_position();
            vec2(mx, my)
        };
        let prod = &self.prod[TEAM_PLAYER as usize];
        for (rect, kind) in self.build_buttons() {
            let hover = rect.contains(mp);
            let afford = self.free_build || self.credits[0] >= unit_stats(kind).cost;
            let bg = if hover {
                Color::new(0.18, 0.22, 0.28, 1.0)
            } else {
                Color::new(0.13, 0.15, 0.18, 1.0)
            };
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
            let border = if afford { team_color(TEAM_PLAYER) } else { Color::new(0.45, 0.22, 0.22, 1.0) };
            draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, border);
            let tcol = if afford { WHITE } else { Color::new(0.7, 0.5, 0.5, 1.0) };
            txt(&format!("{}  {}", kind.hotkey(), self.t(kind.name_key())), rect.x + 8.0, rect.y + 18.0, 17.0, tcol);
            txt(&format!("${}", unit_stats(kind).cost as i32), rect.x + 8.0, rect.y + 38.0, 16.0, Color::new(0.90, 0.82, 0.32, 1.0));
            // Antall = i ko + under bygging (alle byggeplasser).
            let qn = prod.queue.iter().filter(|k| **k == kind).count()
                + prod.active.iter().filter(|(k, _)| *k == kind).count();
            if qn > 0 {
                txt(&format!("x{}", qn), rect.x + rect.w - 28.0, rect.y + 18.0, 17.0, Color::new(0.6, 0.9, 1.0, 1.0));
            }
            // Fremdrift = den mest ferdige aktive byggeplassen for denne typen.
            let total = unit_stats(kind).build_time;
            let pct = prod
                .active
                .iter()
                .filter(|(k, _)| *k == kind)
                .map(|(_, t)| 1.0 - (t / total).clamp(0.0, 1.0))
                .fold(0.0_f32, f32::max);
            if pct > 0.0 {
                draw_rectangle(rect.x, rect.y + rect.h - 5.0, rect.w * pct, 5.0, Color::new(0.30, 0.80, 0.40, 1.0));
            }
        }

        // --- Bygninger i kategorier (plasseres pa kartet) ---
        let (headers, bbtns) = self.building_menu();
        for (hy, name) in headers {
            txt(self.t(name), x + 10.0, hy + 11.0, 14.0, Color::new(0.7, 0.8, 0.95, 0.95));
        }
        for (rect, kind) in bbtns {
            let hover = rect.contains(mp);
            let afford = self.free_build || self.credits[0] >= kind.cost();
            let active = self.placing == Some(kind) && self.move_src.is_none();
            let bg = if active {
                Color::new(0.16, 0.30, 0.20, 1.0)
            } else if hover {
                Color::new(0.18, 0.22, 0.28, 1.0)
            } else {
                Color::new(0.13, 0.15, 0.18, 1.0)
            };
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
            let border = if active {
                GREEN
            } else if afford {
                team_color(TEAM_PLAYER)
            } else {
                Color::new(0.45, 0.22, 0.22, 1.0)
            };
            draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, border);
            let tcol = if afford { WHITE } else { Color::new(0.7, 0.5, 0.5, 1.0) };
            txt(self.t(kind.name_key()), rect.x + 8.0, rect.y + 16.0, 15.0, tcol);
            txt(&format!("${}", kind.cost() as i32), rect.x + 8.0, rect.y + 32.0, 14.0, Color::new(0.90, 0.82, 0.32, 1.0));
            if active {
                txt(self.t(Key::Placing), rect.x + rect.w - 56.0, rect.y + 32.0, 13.0, Color::new(0.6, 0.95, 0.6, 1.0));
            }
        }

        // --- Handlinger for valgt bygning: Flytt / Fjern ---
        if let Some((flytt, reparer, fjern)) = self.building_action_buttons() {
            let sel = self
                .buildings
                .iter()
                .find(|b| b.selected && b.team == TEAM_PLAYER && b.kind != BuildingKind::Hq)
                .map(|b| (b.kind, b.hp));
            if let Some((kind, hp)) = sel {
                // Flytt
                let moving = self.move_src.is_some();
                draw_rectangle(flytt.x, flytt.y, flytt.w, flytt.h, if moving { Color::new(0.16, 0.30, 0.20, 1.0) } else { Color::new(0.15, 0.18, 0.24, 1.0) });
                draw_rectangle_lines(flytt.x, flytt.y, flytt.w, flytt.h, 2.0, if moving { GREEN } else { team_color(TEAM_PLAYER) });
                txt(self.t(Key::ActionMove), flytt.x + 6.0, flytt.y + 15.0, 14.0, WHITE);
                txt(&format!("${}", Self::move_cost(kind) as i32), flytt.x + 6.0, flytt.y + 29.0, 12.0, Color::new(0.90, 0.82, 0.32, 1.0));
                // Reparer
                let damaged = hp < kind.max_hp() - 1.0;
                let fee = ((kind.max_hp() - hp) / kind.max_hp() * kind.cost() * 0.5) as i32;
                draw_rectangle(reparer.x, reparer.y, reparer.w, reparer.h, Color::new(0.13, 0.20, 0.16, 1.0));
                draw_rectangle_lines(reparer.x, reparer.y, reparer.w, reparer.h, 2.0, if damaged { Color::new(0.4, 0.85, 0.5, 1.0) } else { Color::new(0.3, 0.4, 0.35, 1.0) });
                txt(self.t(Key::ActionRepair), reparer.x + 6.0, reparer.y + 15.0, 14.0, if damaged { WHITE } else { Color::new(0.6, 0.7, 0.6, 1.0) });
                let rep_label = if damaged { format!("${}", fee) } else { self.t(Key::Full).to_string() };
                txt(&rep_label, reparer.x + 6.0, reparer.y + 29.0, 12.0, Color::new(0.6, 0.9, 0.6, 1.0));
                // Fjern (med bekreftelse)
                let armed = self.confirm_remove;
                draw_rectangle(fjern.x, fjern.y, fjern.w, fjern.h, if armed { Color::new(0.45, 0.12, 0.12, 1.0) } else { Color::new(0.24, 0.14, 0.14, 1.0) });
                draw_rectangle_lines(fjern.x, fjern.y, fjern.w, fjern.h, 2.0, Color::new(0.9, 0.35, 0.3, 1.0));
                if armed {
                    txt(self.t(Key::ConfirmRemove), fjern.x + 8.0, fjern.y + 22.0, 14.0, Color::new(1.0, 0.8, 0.4, 1.0));
                } else {
                    txt(self.t(Key::ActionRemove), fjern.x + 8.0, fjern.y + 16.0, 14.0, WHITE);
                    txt(&format!("{} +${}", self.t(Key::Refund), (kind.cost() * 0.5) as i32), fjern.x + 8.0, fjern.y + 30.0, 12.0, Color::new(0.6, 0.9, 0.6, 1.0));
                }
            }
        }

        // (DevHint-teksten skjult bevisst -- den var i veien for flaggvelgeren.)
    }

    fn draw_minimap(&self) {
        self.draw_minimap_at(self.minimap_rect());
    }

    // Flytende minikart (nede til venstre) som dukker opp mens man navigerer og
    // byggmenyen er lukket -- viser kartet + markering av synlig utsnitt.
    pub(crate) fn nav_minimap_rect(&self) -> Rect {
        let w = 132.0_f32.min(self.play_w() - 20.0);
        let h = w * (MAP_H as f32 / MAP_W as f32);
        Rect::new(10.0, screen_height() - h - 12.0 - self.safe_bottom(), w, h)
    }
    pub(crate) fn nav_minimap_visible(&self) -> bool {
        self.screen == Screen::Playing && !self.sidebar_on() && self.nav_show > 0.0
    }
    pub(crate) fn draw_nav_minimap(&self) {
        if !self.nav_minimap_visible() {
            return;
        }
        let mm = self.nav_minimap_rect();
        let a = (self.nav_show / 0.4).clamp(0.0, 1.0); // ton raskt ut nar man slutter a navigere
        draw_rectangle(mm.x - 4.0, mm.y - 4.0, mm.w + 8.0, mm.h + 8.0, Color::new(0.05, 0.06, 0.07, 0.82 * a));
        draw_rectangle_lines(mm.x - 4.0, mm.y - 4.0, mm.w + 8.0, mm.h + 8.0, 1.5, Color::new(0.30, 0.55, 0.40, 0.9 * a));
        self.draw_minimap_at(mm);
    }

    fn draw_minimap_at(&self, mm: Rect) {
        self.draw_minimap_inner(mm, false, true);
    }

    // `reveal_all` = ignorer take (kun dev/reveal). `show_viewport` = tegn kamera-
    // utsnittet. Kart-previewen bruker take (reveal_all=false) sa man IKKE ser
    // fiendebasen, og uten kamera-rute.
    fn draw_minimap_inner(&self, mm: Rect, reveal_all: bool, show_viewport: bool) {
        let full = reveal_all || self.reveal;
        let map_px = vec2(MAP_W as f32 * TILE, MAP_H as f32 * TILE);
        draw_rectangle(mm.x - 2.0, mm.y - 2.0, mm.w + 4.0, mm.h + 4.0, BLACK);
        let sx = mm.w / MAP_W as f32;
        let sy = mm.h / MAP_H as f32;
        for ty in 0..MAP_H {
            for tx in 0..MAP_W {
                let idx = ty * MAP_W + tx;
                if !full && !self.explored[idx] {
                    continue;
                }
                let mut c = self.map[idx].color(tx, ty);
                if !full && !self.visible[idx] {
                    c = Color::new(c.r * 0.55, c.g * 0.55, c.b * 0.55, 1.0);
                }
                draw_rectangle(mm.x + tx as f32 * sx, mm.y + ty as f32 * sy, sx + 0.6, sy + 0.6, c);
            }
        }
        let br = (sx * 0.55).max(3.5); // prikk-storrelse skalerer med oppløsning
        let ur = (sx * 0.40).max(2.0);
        for b in &self.buildings {
            if !full && b.team != TEAM_PLAYER && !self.tile_visible(b.pos) {
                continue;
            }
            let mx = mm.x + (b.pos.x / map_px.x) * mm.w;
            let my = mm.y + (b.pos.y / map_px.y) * mm.h;
            draw_rectangle(mx - br * 0.5, my - br * 0.5, br, br, team_color(b.team));
        }
        for u in &self.units {
            if !full && u.team != TEAM_PLAYER && !self.tile_visible(u.pos) {
                continue;
            }
            let mx = mm.x + (u.pos.x / map_px.x) * mm.w;
            let my = mm.y + (u.pos.y / map_px.y) * mm.h;
            draw_rectangle(mx - ur * 0.5, my - ur * 0.5, ur, ur, team_color(u.team));
        }
        if show_viewport {
            // synlig kamera-utsnitt
            let view = vec2(self.play_w(), screen_height()) / self.zoom;
            let vx = mm.x + (self.cam.x / map_px.x) * mm.w;
            let vy = mm.y + (self.cam.y / map_px.y) * mm.h;
            let vw = (view.x / map_px.x) * mm.w;
            let vh = (view.y / map_px.y) * mm.h;
            draw_rectangle_lines(vx, vy, vw, vh, 1.5, WHITE);
        }
    }

    // Tikk den levende kart-previewen bak nivavelgeren. Hover over en nivaknapp
    // bytter niva; simuleringen kjorer ~10 s og looper. Lyd dempes.
    pub(crate) fn update_preview(&mut self, dt: f32) {
        if self.screen != Screen::Start {
            self.preview = None;
            return;
        }
        // Hvilket niva? Det man holder over, ellers det forrige (start: niva 1).
        let (mx, my) = mouse_position();
        let m = vec2(mx, my);
        let mut target = self.preview_level.min(self.max_unlocked);
        for i in 0..(self.max_unlocked + 1).min(levels::count()) {
            if self.menu_level_rect(i).contains(m) {
                target = i;
                break;
            }
        }
        let need_new = self.preview.as_ref().map_or(true, |_| self.preview_level != target);
        if need_new {
            self.preview_level = target;
            self.preview_time = 0.0;
            self.preview = Some(Box::new(Game::new_level(target)));
        }
        // Tikk simuleringen (lydlost), loop hver 10 s.
        let was_muted = bridge::is_muted();
        bridge::set_muted(true);
        if let Some(g) = self.preview.as_mut() {
            g.update(dt);
        }
        self.preview_time += dt;
        if self.preview_time > 10.0 {
            self.preview = Some(Box::new(Game::new_level(self.preview_level)));
            self.preview_time = 0.0;
        }
        bridge::set_muted(was_muted);
    }

    pub(crate) fn draw_preview_bg(&self) {
        if let Some(g) = self.preview.as_ref() {
            let aspect = MAP_W as f32 / MAP_H as f32;
            let (sw, sh) = (screen_width(), screen_height());
            // Litt innzoomet sa basen fyller mer (dekker minst skjermen).
            let mut w = sw * 1.25;
            let mut h = w / aspect;
            if h < sh * 1.25 {
                h = sh * 1.25;
                w = h * aspect;
            }
            // Sentrer pa spillerbasen (forskyv rekta sa basen havner i midten).
            let base = levels::get(self.preview_level).player_base;
            let map_px = vec2(MAP_W as f32 * TILE, MAP_H as f32 * TILE);
            let fx = base.0 as f32 * TILE / map_px.x;
            let fy = base.1 as f32 * TILE / map_px.y;
            let r = Rect::new(sw * 0.5 - fx * w, sh * 0.5 - fy * h, w, h);
            // Take pa (reveal_all=false) -> kun var base/utforsket, fiendebasen
            // forblir svart. Ingen kamera-rute.
            g.draw_minimap_inner(r, false, false);
            // Dempende slor sa menyteksten er lesbar.
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.04, 0.05, 0.07, 0.50));
        }
    }

    fn draw_hud(&self) {
        let players = self.units.iter().filter(|u| u.team == TEAM_PLAYER).count();
        let w = self.play_w();

        draw_rectangle(0.0, 0.0, w, 30.0, Color::new(0.0, 0.0, 0.0, 0.72));
        draw_rectangle(0.0, 30.0, w, 1.5, team_color(TEAM_PLAYER));
        // kreditt-mynt
        draw_circle(20.0, 15.0, 7.0, Color::new(0.92, 0.80, 0.25, 1.0));
        draw_circle_lines(20.0, 15.0, 7.0, 1.5, Color::new(0.55, 0.45, 0.10, 1.0));
        txt(
            &format!(
                "{}    {}: {}{}{}{}",
                self.credits[0] as i32,
                self.t(Key::Units),
                players,
                if self.paused { "   [PAUSE]" } else { "" },
                if self.free_build { "  [GRATIS]" } else { "" },
                if self.reveal { "  [AVSLORT]" } else { "" },
            ),
            34.0,
            21.0,
            21.0,
            WHITE,
        );

        // (Hjelpelinjen "venstre-dra: velg ..." skjult bevisst -- folk finner
        // ut av styringen ved a prove.)
        // Seier/tap-panelet tegnes i ui.rs (draw_controls) med klikkbare knapper.
    }
}

// ---------------------------------------------------------------------------
// Hovedlokke
// ---------------------------------------------------------------------------

fn window_conf() -> Conf {
    Conf {
        window_title: "OpenRA Rust (WebGL)".to_owned(),
        window_width: 1280,
        window_height: 720,
        // high_dpi PA: macroquad normaliserer screen_width()/mouse_position()
        // med dpi_scale(), sa koordinatene vare er logiske (CSS-px) uansett --
        // men teksten rasteres ved dpr x for skarp gjengivelse pa retina/iPhone.
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // Last Unicode-font (æøå, gresk, kyrillisk, CJK, arabisk osv.). Faller
    // tilbake til standardfonten hvis filen mangler.
    if let Ok(font) = load_ttf_font("font.ttf").await {
        set_font(font);
    }

    let mut game = Game::new();

    loop {
        let dt = get_frame_time().min(0.05);

        if is_key_pressed(KeyCode::R) && game.screen == Screen::Playing {
            let lvl = game.level; // spill gjeldende niva pa nytt
            game.load_level(lvl);
        }

        game.handle_ui();
        if game.screen == Screen::Playing {
            game.handle_camera(dt);
            game.handle_keys();
            game.handle_selection();
            game.update(dt);
            // Vis flytende minikart mens man navigerer + en kort hale (~1 s).
            if (game.cam - game.prev_cam).length() > 0.4 {
                game.nav_show = 1.0;
            }
            game.prev_cam = game.cam;
            if game.nav_show > 0.0 {
                game.nav_show -= dt;
            }
            if game.rally_show > 0.0 {
                game.rally_show -= dt;
            }
            if let Some((_, t)) = &mut game.move_marker {
                *t -= dt;
                if *t <= 0.0 {
                    game.move_marker = None;
                }
            }
        } else {
            // Levende kart-preview bak nivavelgeren.
            game.update_preview(dt);
        }
        game.draw();

        let players = game.units.iter().filter(|u| u.team == TEAM_PLAYER).count() as i32;
        let enemies = game.units.iter().filter(|u| u.team == TEAM_ENEMY).count() as i32;
        let selected = game.units.iter().filter(|u| u.selected).count() as i32
            + game.buildings.iter().filter(|b| b.selected).count() as i32;
        let outcome_code = match game.outcome {
            None => 0,
            Some(true) => 1,
            Some(false) => 2,
        };
        bridge::report(
            game.cam.x, game.cam.y, game.zoom, game.last_mouse.x, game.last_mouse.y,
            game.mouse_active, players, enemies, selected, get_fps(), outcome_code,
        );
        let p = &game.prod[TEAM_PLAYER as usize];
        // Mest ferdige aktive bygg -> prosent; ko-lengde inkluderer aktive bygg.
        let queue_pct = p
            .active
            .iter()
            .map(|(k, t)| ((1.0 - t / unit_stats(*k).build_time) * 100.0) as i32)
            .max()
            .unwrap_or(0);
        let queue_len = (p.queue.len() + p.active.len()) as i32;
        let flags = (game.paused as i32)
            | ((game.free_build as i32) << 1)
            | ((game.god_mode as i32) << 2)
            | ((game.reveal as i32) << 3);
        bridge::report_econ(
            game.credits[0] as i32, game.credits[1] as i32,
            game.count_buildings(TEAM_PLAYER), game.count_buildings(TEAM_ENEMY),
            queue_len, queue_pct, game.speed, flags,
        );
        let explored = game.explored.iter().filter(|&&e| e).count();
        let visible = game.visible.iter().filter(|&&v| v).count();
        let pct = (explored * 100 / (MAP_W * MAP_H)) as i32;
        bridge::report_fog(pct, visible as i32, game.reveal);

        // Tom kommando-koen (med tak for sikkerhets skyld).
        for _ in 0..64 {
            let (code, args) = bridge::poll();
            if code == 0 {
                break;
            }
            game.apply_command(code, args);
        }

        next_frame().await;
    }
}

// ---------------------------------------------------------------------------
// Tester
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn astar_router_rundt_vegg() {
        // Vegg pa kolonne x=10 for alle y unntatt overste rad (apning oppe).
        let mut blocked = vec![false; MAP_W * MAP_H];
        for y in 1..MAP_H {
            blocked[y * MAP_W + 10] = true;
        }
        let start = tile_center(5, 20);
        let goal = tile_center(20, 20);
        let path = astar(&blocked, start, goal);
        assert!(!path.is_empty(), "skal finne en sti rundt veggen");
        for w in &path {
            let tx = (w.x / TILE).floor() as usize;
            let ty = (w.y / TILE).floor() as usize;
            assert!(!blocked[ty * MAP_W + tx], "waypoint havnet i blokkert rute");
        }
        let last = *path.last().unwrap();
        assert!(last.distance(goal) < TILE * 1.5, "siste waypoint skal vaere ved malet");
    }

    #[test]
    fn ref_aldri_pa_gull_og_malm_naaes_for_alle_niva() {
        // For hvert niva: spillerens raffineri skal ALDRI sta oppa malm (da satte
        // harvesteren seg fast uten a hente/levere), fotavtrykket skal vaere
        // ryddet til gress, og minst en malm-rute skal vaere fremkommelig fra
        // raffineriet sa hosting faktisk fungerer.
        for lvl in 0..levels::count() {
            let g = Game::new_level(lvl);
            let refb = g
                .buildings
                .iter()
                .find(|b| b.team == TEAM_PLAYER && b.kind == BuildingKind::Refinery)
                .expect("spiller skal ha raffineri");
            let r = refb.kind.radius();
            let minx = ((refb.pos.x - r) / TILE).floor() as i32;
            let maxx = ((refb.pos.x + r) / TILE).floor() as i32;
            let miny = ((refb.pos.y - r) / TILE).floor() as i32;
            let maxy = ((refb.pos.y + r) / TILE).floor() as i32;
            for ty in miny..=maxy {
                for tx in minx..=maxx {
                    if in_bounds(tx, ty) {
                        let idx = ty as usize * MAP_W + tx as usize;
                        assert!(g.ore[idx] <= 0.0, "niva {}: raffineriet star oppa malm i ({},{})", lvl + 1, tx, ty);
                        assert!(g.map[idx] == Terrain::Grass, "niva {}: raffineri-fotavtrykk ikke ryddet i ({},{})", lvl + 1, tx, ty);
                    }
                }
            }
            // BFS over fremkommelig terreng fra raffineri-ruta -> na minst en malm.
            let rx = (refb.pos.x / TILE).floor() as i32;
            let ry = (refb.pos.y / TILE).floor() as i32;
            let mut seen = vec![false; MAP_W * MAP_H];
            let mut q = std::collections::VecDeque::new();
            seen[ry as usize * MAP_W + rx as usize] = true;
            q.push_back((rx, ry));
            let mut found_ore = false;
            while let Some((x, y)) = q.pop_front() {
                if g.ore[y as usize * MAP_W + x as usize] > 0.0 {
                    found_ore = true;
                    break;
                }
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if in_bounds(nx, ny) {
                        let ni = ny as usize * MAP_W + nx as usize;
                        if !seen[ni] && g.map[ni].passable() {
                            seen[ni] = true;
                            q.push_back((nx, ny));
                        }
                    }
                }
            }
            assert!(found_ore, "niva {}: ingen malm er fremkommelig fra raffineriet", lvl + 1);
        }
    }

    #[test]
    fn astar_blokkert_mal_flyttes_til_naboruta() {
        // Mal-ruta er blokkert (som en bygning), men naboene er apne.
        let mut blocked = vec![false; MAP_W * MAP_H];
        let (gx, gy) = (20, 20);
        blocked[gy * MAP_W + gx] = true;
        let path = astar(&blocked, tile_center(5, 20), tile_center(gx as i32, gy as i32));
        assert!(!path.is_empty(), "skal finne sti til naermeste apne rute");
        let last = *path.last().unwrap();
        assert!(last.distance(tile_center(gx as i32, gy as i32)) < TILE * 2.5);
    }

    #[test]
    fn astar_ingen_sti_nar_alt_blokkert() {
        // Hele kartet blokkert -> ingen apen rute -> ingen sti.
        let blocked = vec![true; MAP_W * MAP_H];
        let path = astar(&blocked, tile_center(5, 20), tile_center(20, 20));
        assert!(path.is_empty(), "skal ikke finne sti nar alt er blokkert");
    }

    #[test]
    fn astar_omringet_mal_rutes_til_kanten() {
        // Blokkert mal omringet -> stien skal ende pa naermeste apne rute, ikke inni.
        let mut blocked = vec![false; MAP_W * MAP_H];
        let (gx, gy) = (20i32, 20i32);
        for dy in -1..=1 {
            for dx in -1..=1 {
                blocked[(gy + dy) as usize * MAP_W + (gx + dx) as usize] = true;
            }
        }
        let path = astar(&blocked, tile_center(5, 20), tile_center(gx, gy));
        assert!(!path.is_empty(), "skal rute til kanten av hindringen");
        let last = *path.last().unwrap();
        let tx = (last.x / TILE).floor() as usize;
        let ty = (last.y / TILE).floor() as usize;
        assert!(!blocked[ty * MAP_W + tx], "skal ikke ende inni hindringen");
    }

    #[test]
    fn astar_samme_rute_gir_tom_sti() {
        let blocked = vec![false; MAP_W * MAP_H];
        let path = astar(&blocked, tile_center(7, 7), tile_center(7, 7));
        assert!(path.is_empty());
    }

    #[test]
    fn score_forutsagt_ko_senker_verdien() {
        // Samme felt, men forutsagt kotid skal gi lavere score.
        let uten = harvest_score(200.0, 3.0, 200.0, 0.0, 300.0);
        let med = harvest_score(200.0, 3.0, 200.0, 5.0, 300.0);
        assert!(med < uten, "kotid skal senke scoren");
    }

    #[test]
    fn score_naermere_felt_er_bedre() {
        let naer = harvest_score(100.0, 3.0, 100.0, 0.0, 300.0);
        let fjern = harvest_score(600.0, 3.0, 600.0, 0.0, 300.0);
        assert!(naer > fjern, "kortere reisetid skal gi hoyere score");
    }

    #[test]
    fn score_mer_levert_er_bedre() {
        let lite = harvest_score(200.0, 3.0, 200.0, 0.0, 100.0);
        let mye = harvest_score(200.0, 3.0, 200.0, 0.0, 300.0);
        assert!(mye > lite, "mer levert malm skal gi hoyere score");
    }

    /// Symmetrisk testbrett: to like malmfelt, hvert med sitt EGET raffineri, og
    /// et midtpunkt like langt fra begge. Da gir forutsagt ko ved ett raffineri
    /// utslag pa feltvalget.
    const MID: Vec2 = Vec2::new(1024.0, 650.0);

    fn symmetric_setup() -> (Game, Vec2, Vec2) {
        let mut g = Game::new();
        g.units.clear();
        for o in g.ore.iter_mut() {
            *o = 0.0;
        }
        // Speilsymmetriske felt: A rundt (12..13,15..16), B rundt (50..51,15..16).
        for &(bx, by) in &[(12usize, 15usize), (50usize, 15usize)] {
            for dy in 0..2 {
                for dx in 0..2 {
                    g.ore[(by + dy) * MAP_W + (bx + dx)] = ORE_PER_TILE;
                }
            }
        }
        g.buildings.clear();
        g.buildings.push(Building::new(vec2(416.0, 200.0), TEAM_PLAYER, BuildingKind::Refinery));
        g.buildings.push(Building::new(vec2(1632.0, 200.0), TEAM_PLAYER, BuildingKind::Refinery));
        let fields = g.ore_fields();
        let a = fields[0].centroid;
        let b = fields[1].centroid;
        (g, a, b)
    }

    #[test]
    fn host_velger_naert_felt_uten_konkurrent() {
        let (g, a, b) = symmetric_setup();
        let t = g.assign_harvester(TEAM_PLAYER, MID, usize::MAX).unwrap();
        assert!(t.distance(a) < t.distance(b), "uten konkurrent skal forste felt velges");
    }

    #[test]
    fn host_unngar_forutsagt_ko() {
        let (mut g, a, b) = symmetric_setup();
        // En host pa vei mot felt A med ankomst som sammenfaller med var egen ->
        // forutsagt ko ved raffineriet skal vri valget til felt B.
        let mut h0 = Unit::new(MID, TEAM_PLAYER, UnitKind::Harvester);
        h0.harv = HarvState::ToOre;
        h0.target = Some(a);
        g.units.push(h0);
        let t = g.assign_harvester(TEAM_PLAYER, MID, usize::MAX).unwrap();
        assert!(
            t.distance(b) < t.distance(a),
            "med forutsagt ko ved A skal host velge B (t={:?})",
            t
        );
    }

    #[test]
    fn okonomi_host_leverer_kreditter() {
        let mut g = Game::new();
        let start = g.credits[TEAM_PLAYER as usize];
        // Simuler 90 spill-sekunder.
        for _ in 0..(90 * 60) {
            g.update(1.0 / 60.0);
        }
        assert!(
            g.credits[TEAM_PLAYER as usize] > start,
            "host skal levere kreditter: {} -> {}",
            start,
            g.credits[TEAM_PLAYER as usize]
        );
    }

    // ----- Markering / flytting -----

    #[test]
    fn flytt_markerte_enheter_beholder_markering() {
        let mut g = Game::new();
        g.clear_selection();
        let idx = g
            .units
            .iter()
            .position(|u| u.team == TEAM_PLAYER && u.kind != UnitKind::Harvester)
            .unwrap();
        g.units[idx].selected = true;
        assert!(g.has_player_selection());
        let dest = g.units[idx].pos + vec2(300.0, 0.0);
        g.move_selected(dest);
        assert!(g.units[idx].selected, "enheten skal forbli markert etter flyttordre");
        assert!(g.units[idx].target.is_some(), "enheten skal ha faatt et mal");
    }

    #[test]
    fn flytt_uten_markering_gjor_ingenting() {
        let mut g = Game::new();
        g.clear_selection();
        assert!(!g.has_player_selection());
        let before: Vec<_> = g.units.iter().map(|u| u.target).collect();
        g.move_selected(vec2(500.0, 500.0));
        let after: Vec<_> = g.units.iter().map(|u| u.target).collect();
        assert_eq!(before, after, "ingen markering -> ingen flyttordre");
    }

    #[test]
    fn parkert_enhet_blokkerer_ikke_bevegende() {
        // En bevegende enhet skal komme forbi en parkert (som viker).
        let mut g = Game::new();
        g.units.clear();
        let parked = vec2(500.0, 500.0);
        g.units.push(Unit::new(parked, TEAM_PLAYER, UnitKind::Tank)); // parkert (ingen sti)
        let mut mover = Unit::new(vec2(560.0, 500.0), TEAM_PLAYER, UnitKind::Tank);
        mover.target = Some(vec2(300.0, 500.0));
        mover.path = vec![vec2(300.0, 500.0)];
        g.units.push(mover);
        for _ in 0..180 {
            g.update(1.0 / 60.0);
        }
        assert!(g.units[1].pos.x < 420.0, "bevegende enhet skal komme forbi, x={}", g.units[1].pos.x);
        assert!(g.units[0].pos.distance(parked) > 3.0, "parkert enhet skal ha veket");
    }

    #[test]
    fn enheter_loser_vranglas_ansikt_til_ansikt() {
        // To enheter rett mot hverandre skal sidesteppe og passere, ikke lase seg.
        let mut g = Game::new();
        g.units.clear();
        let mut a = Unit::new(vec2(540.0, 500.0), TEAM_PLAYER, UnitKind::Tank);
        a.target = Some(vec2(200.0, 500.0));
        a.path = vec![vec2(200.0, 500.0)];
        let mut b = Unit::new(vec2(560.0, 500.0), TEAM_PLAYER, UnitKind::Tank);
        b.target = Some(vec2(900.0, 500.0));
        b.path = vec![vec2(900.0, 500.0)];
        g.units.push(a);
        g.units.push(b);
        for _ in 0..180 {
            g.update(1.0 / 60.0);
        }
        assert!(g.units[0].pos.x < 480.0, "A skal komme forbi mot venstre, x={}", g.units[0].pos.x);
        assert!(g.units[1].pos.x > 620.0, "B skal komme forbi mot hoyre, x={}", g.units[1].pos.x);
    }

    // ----- Bygningsplassering -----

    #[test]
    fn bygning_plasseres_og_trekker_kreditter() {
        let mut g = Game::new();
        for t in g.map.iter_mut() {
            *t = Terrain::Grass;
        }
        g.buildings.clear();
        g.credits[TEAM_PLAYER as usize] = 5000.0;
        let n0 = g.buildings.len();
        assert!(g.place_building(BuildingKind::Refinery, vec2(1000.0, 1000.0)), "skal kunne bygge pa gress");
        assert_eq!(g.buildings.len(), n0 + 1);
        assert_eq!(g.credits[TEAM_PLAYER as usize], 5000.0 - BuildingKind::Refinery.cost());
    }

    #[test]
    fn bygning_nektes_ved_overlapp_vann_og_for_lite_penger() {
        let mut g = Game::new();
        for t in g.map.iter_mut() {
            *t = Terrain::Grass;
        }
        g.buildings.clear();
        g.credits[TEAM_PLAYER as usize] = 9999.0;
        assert!(g.place_building(BuildingKind::Factory, vec2(1000.0, 1000.0)));
        assert!(!g.place_building(BuildingKind::Factory, vec2(1010.0, 1000.0)), "overlapp skal nektes");
        // vann under fotavtrykket
        let center = vec2(1500.0, 1000.0);
        let tx = (center.x / TILE) as usize;
        let ty = (center.y / TILE) as usize;
        g.map[ty * MAP_W + tx] = Terrain::Water;
        assert!(!g.place_building(BuildingKind::Refinery, center), "vann skal nektes");
        // for lite penger
        g.credits[TEAM_PLAYER as usize] = 10.0;
        assert!(!g.place_building(BuildingKind::Refinery, vec2(2000.0, 1500.0)), "for lite penger skal nektes");
    }

    #[test]
    fn gjerde_blokkerer_ruter() {
        let mut g = Game::new();
        for t in g.map.iter_mut() {
            *t = Terrain::Grass;
        }
        g.buildings.clear();
        let pos = vec2(1000.0, 1000.0);
        g.buildings.push(Building::new(pos, TEAM_PLAYER, BuildingKind::Wall));
        let blocked = g.compute_blocked();
        let tx = (pos.x / TILE) as usize;
        let ty = (pos.y / TILE) as usize;
        assert!(blocked[ty * MAP_W + tx], "gjerde skal blokkere ruten sin");
    }

    #[test]
    fn flytt_bygning_doemmer_gammel_og_reiser_ny() {
        let mut g = Game::new();
        for t in g.map.iter_mut() {
            *t = Terrain::Grass;
        }
        g.buildings.clear();
        g.credits[TEAM_PLAYER as usize] = 5000.0;
        assert!(g.place_building(BuildingKind::Refinery, vec2(1000.0, 1000.0)));
        let src = g.buildings.last().unwrap().pos;
        let c1 = g.credits[TEAM_PLAYER as usize];
        assert!(g.relocate_building(BuildingKind::Refinery, src, vec2(1500.0, 1000.0)));
        assert!(
            g.buildings.iter().any(|b| b.pos.distance(vec2(1500.0, 1000.0)) < TILE && !b.condemned),
            "ny bygning skal reises"
        );
        assert!(
            g.buildings.iter().any(|b| b.pos.distance(src) < 1.0 && b.condemned),
            "gammel bygning skal doemmes til riving"
        );
        assert_eq!(g.credits[TEAM_PLAYER as usize], c1 - Game::move_cost(BuildingKind::Refinery));
    }

    #[test]
    fn tarn_skyter_fiende() {
        let mut g = Game::new();
        for t in g.map.iter_mut() {
            *t = Terrain::Grass;
        }
        g.buildings.clear();
        g.units.clear();
        g.buildings.push(Building::new(vec2(1000.0, 1000.0), TEAM_PLAYER, BuildingKind::Hq));
        g.buildings.push(Building::new(vec2(2000.0, 1000.0), TEAM_ENEMY, BuildingKind::Hq));
        let tp = vec2(1300.0, 1000.0);
        g.buildings.push(Building::new(tp, TEAM_PLAYER, BuildingKind::Turret));
        let mut e = Unit::new(tp + vec2(120.0, 0.0), TEAM_ENEMY, UnitKind::Rifleman);
        e.hp = 45.0;
        g.units.push(e);
        for _ in 0..240 {
            g.update(1.0 / 60.0); // ~4 s -> nok skudd til a felle infanteriet
        }
        let alive = g.units.iter().filter(|u| u.team == TEAM_ENEMY).count();
        assert_eq!(alive, 0, "tarnet skal ha nedkjempet fienden");
    }

    #[test]
    fn doemt_bygg_rives_av_egne_enheter() {
        let mut g = Game::new();
        for t in g.map.iter_mut() {
            *t = Terrain::Grass;
        }
        g.buildings.clear();
        g.units.clear();
        g.buildings.push(Building::new(vec2(1000.0, 1000.0), TEAM_PLAYER, BuildingKind::Hq));
        g.buildings.push(Building::new(vec2(2000.0, 1000.0), TEAM_ENEMY, BuildingKind::Hq));
        let wallpos = vec2(1300.0, 1000.0);
        g.buildings.push(Building::new(wallpos, TEAM_PLAYER, BuildingKind::Wall));
        let widx = g.buildings.len() - 1;
        g.units.push(Unit::new(wallpos + vec2(45.0, 0.0), TEAM_PLAYER, UnitKind::Tank));
        let hp0 = g.buildings[widx].hp;
        g.condemn_building(widx);
        assert!(g.buildings[widx].condemned);
        for _ in 0..120 {
            g.update(1.0 / 60.0);
        }
        let demolished = match g.buildings.iter().position(|b| b.kind == BuildingKind::Wall) {
            None => true,
            Some(i) => g.buildings[i].hp < hp0,
        };
        assert!(demolished, "egne enheter skal rive det doemte gjerdet");
    }

    #[test]
    fn ny_bygning_ikke_operativ_for_gammel_revet() {
        let mut g = Game::new();
        for t in g.map.iter_mut() {
            *t = Terrain::Grass;
        }
        g.buildings.clear();
        g.units.clear();
        g.free_build = true;
        g.place_building(BuildingKind::Factory, vec2(1000.0, 1000.0));
        let src = g.buildings.last().unwrap().pos;
        assert_eq!(g.factory_count(TEAM_PLAYER), 1);
        assert!(g.relocate_building(BuildingKind::Factory, src, vec2(1500.0, 1000.0)));
        // Ny fabrikk venter -> teller ikke som operativ enda.
        assert_eq!(g.factory_count(TEAM_PLAYER), 1, "ny fabrikk skal ikke vaere operativ");
        assert!(g.buildings.iter().any(|b| b.awaiting.is_some()), "ny skal vente pa riving");
        // Riv den gamle (sett HP=0) og kjor en oppdatering.
        let oldidx = g.buildings.iter().position(|b| b.condemned).unwrap();
        g.buildings[oldidx].hp = 0.0;
        g.update(1.0 / 60.0);
        assert!(g.buildings.iter().all(|b| b.awaiting.is_none()), "ny skal bli operativ");
        assert_eq!(g.factory_count(TEAM_PLAYER), 1);
    }

    #[test]
    fn tarn_avfyrer_homing_prosjektil() {
        let mut g = Game::new();
        for t in g.map.iter_mut() {
            *t = Terrain::Grass;
        }
        g.buildings.clear();
        g.units.clear();
        g.buildings.push(Building::new(vec2(1000.0, 1000.0), TEAM_PLAYER, BuildingKind::Hq));
        g.buildings.push(Building::new(vec2(2000.0, 1000.0), TEAM_ENEMY, BuildingKind::Hq));
        let tp = vec2(1300.0, 1000.0);
        g.buildings.push(Building::new(tp, TEAM_PLAYER, BuildingKind::Turret));
        g.units.push(Unit::new(tp + vec2(120.0, 0.0), TEAM_ENEMY, UnitKind::Rifleman));
        g.update(1.0 / 60.0); // tarnet fyrer pa forste tick (cooldown 0)
        assert!(!g.projectiles.is_empty(), "tarnet skal avfyre en storkule");
    }

    #[test]
    fn reparer_gjenoppretter_hp_mot_gebyr() {
        let mut g = Game::new();
        g.buildings.clear();
        g.credits[TEAM_PLAYER as usize] = 5000.0;
        // Behold HK-er for begge lag sa spillet ikke avsluttes (update stopper da).
        g.buildings.push(Building::new(vec2(500.0, 500.0), TEAM_PLAYER, BuildingKind::Hq));
        g.buildings.push(Building::new(vec2(3000.0, 3000.0), TEAM_ENEMY, BuildingKind::Hq));
        g.buildings.push(Building::new(vec2(1000.0, 1000.0), TEAM_PLAYER, BuildingKind::Turret));
        let idx = g.buildings.len() - 1;
        g.buildings[idx].hp = 100.0;
        let c0 = g.credits[TEAM_PLAYER as usize];
        // Umiddelbar reparasjon: full HP med en gang, mot et gebyr.
        assert!(g.repair_building(idx));
        assert_eq!(g.buildings[idx].hp, BuildingKind::Turret.max_hp(), "skal bli full umiddelbart");
        assert!(g.credits[TEAM_PLAYER as usize] < c0, "reparasjon skal koste");
        assert!(!g.repair_building(idx), "full bygning kan ikke repareres");
    }

    #[test]
    fn flere_fabrikker_bygger_parallelt() {
        let mut g = Game::new();
        for t in g.map.iter_mut() {
            *t = Terrain::Grass;
        }
        g.free_build = true;
        let f0 = g.factory_count(TEAM_PLAYER);
        g.buildings.push(Building::new(vec2(1000.0, 1000.0), TEAM_PLAYER, BuildingKind::Factory));
        assert_eq!(g.factory_count(TEAM_PLAYER), f0 + 1);
        g.build(TEAM_PLAYER, UnitKind::Tank);
        g.build(TEAM_PLAYER, UnitKind::Tank);
        let p0 = g.count_units(TEAM_PLAYER, UnitKind::Tank);
        // Litt over EN byggetid: med to fabrikker skal begge bli ferdige.
        for _ in 0..((unit_stats(UnitKind::Tank).build_time * 60.0) as i32 + 60) {
            g.update(1.0 / 60.0);
        }
        let built = g.count_units(TEAM_PLAYER, UnitKind::Tank) - p0;
        assert!(built >= 2, "to fabrikker skal bygge parallelt (fikk {} pa en byggetid)", built);
    }

    #[test]
    fn bygninger_krever_passasje_mellom_seg() {
        let mut g = Game::new();
        for t in g.map.iter_mut() {
            *t = Terrain::Grass;
        }
        g.buildings.clear();
        g.free_build = true;
        assert!(g.place_building(BuildingKind::Factory, vec2(1000.0, 1000.0)));
        let placed = g.buildings.last().unwrap().pos;
        let rr = BuildingKind::Factory.radius();
        let too_close = placed + vec2(rr * 2.0 + 4.0, 0.0);
        assert!(!g.can_place_building(BuildingKind::Factory, too_close), "for tett skal nektes");
        let with_lane = placed + vec2(rr * 2.0 + TILE * 2.0, 0.0);
        assert!(g.can_place_building(BuildingKind::Factory, with_lane), "med passasje skal tillates");
    }

    #[test]
    fn gjerde_stopper_enhet_fysisk() {
        let mut g = Game::new();
        for t in g.map.iter_mut() {
            *t = Terrain::Grass;
        }
        g.buildings.clear();
        g.units.clear();
        // Behold HK-er sa update() ikke avslutter spillet.
        g.buildings.push(Building::new(vec2(200.0, 200.0), TEAM_PLAYER, BuildingKind::Hq));
        g.buildings.push(Building::new(vec2(3000.0, 3000.0), TEAM_ENEMY, BuildingKind::Hq));
        let wpos = vec2(1000.0, 1000.0);
        g.buildings.push(Building::new(wpos, TEAM_PLAYER, BuildingKind::Wall));
        // Plasser en fiende oppi gjerdets fotavtrykk.
        g.units.push(Unit::new(wpos, TEAM_ENEMY, UnitKind::Tank));
        g.update(1.0 / 60.0);
        let d = g.units[0].pos.distance(wpos);
        assert!(d >= BuildingKind::Wall.radius(), "enhet skal skyves ut av gjerdet, avstand={}", d);
    }

    #[test]
    fn lossing_skjer_gradvis() {
        let mut g = Game::new();
        g.units.clear();
        let r = g
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::Refinery && b.team == TEAM_PLAYER)
            .unwrap()
            .pos;
        let mut h = Unit::new(r, TEAM_PLAYER, UnitKind::Harvester);
        h.harv = HarvState::Unloading;
        h.carrying = HARVEST_CAPACITY;
        h.work_timer = UNLOAD_TIME;
        h.target = Some(r);
        g.units.push(h);
        let c0 = g.credits[TEAM_PLAYER as usize];
        g.update(0.2); // kort steg -> noe levert, ikke alt
        let mid = g.units[0].carrying;
        assert!(mid < HARVEST_CAPACITY && mid > 0.0, "lasten skal synke gradvis, er {}", mid);
        assert!(g.credits[TEAM_PLAYER as usize] > c0, "kreditter skal oke gradvis");
        // kjor lossingen ferdig
        for _ in 0..((UNLOAD_TIME * 60.0) as i32 + 10) {
            g.update(1.0 / 60.0);
        }
        let delivered = g.credits[TEAM_PLAYER as usize] - c0;
        assert!((delivered - HARVEST_CAPACITY).abs() < 5.0, "hele lasten skal leveres, levert={}", delivered);
    }

    // ----- Fiende-AI -----

    #[test]
    fn fiende_bygger_hoster_for_haer() {
        // Fersk start: fienden har 1 host (< onsket 2) og rad -> bygg host forst.
        let g = Game::new();
        assert_eq!(g.count_units(TEAM_ENEMY, UnitKind::Harvester), 1);
        assert_eq!(g.enemy_should_build(), Some(UnitKind::Harvester), "okonomi forst");
    }

    #[test]
    fn fiende_bygger_haer_med_nok_hostere() {
        let mut g = Game::new();
        // Gi fienden den andre hosteren -> na skal den bygge haer.
        g.units.push(Unit::new(vec2(300.0, 300.0), TEAM_ENEMY, UnitKind::Harvester));
        g.credits[TEAM_ENEMY as usize] = 1500.0;
        assert_eq!(g.enemy_should_build(), Some(UnitKind::Tank), "stridsvogn nar rad");
    }

    #[test]
    fn fiende_bygger_infanteri_uten_rad_til_vogn() {
        let mut g = Game::new();
        g.units.push(Unit::new(vec2(300.0, 300.0), TEAM_ENEMY, UnitKind::Harvester));
        g.credits[TEAM_ENEMY as usize] = 300.0; // >= infanteri(100), < vogn(500)
        assert_eq!(g.enemy_should_build(), Some(UnitKind::Rifleman));
    }

    #[test]
    fn niva_med_to_fiender_har_to_hk() {
        // Et to-fiende-niva skal gi to fiende-HK (begge ma rives for seier).
        let idx = levels::LEVELS
            .iter()
            .position(|s| s.enemies.len() == 2)
            .expect("kampanjen har minst ett to-fiende-niva");
        let g = Game::new_level(idx);
        let ehq = g
            .buildings
            .iter()
            .filter(|b| b.team == TEAM_ENEMY && b.kind == BuildingKind::Hq)
            .count();
        assert_eq!(ehq, 2, "to fiendebaser -> to HK");
    }

    #[test]
    fn niva_setter_riktige_kreditter() {
        // Hvert niva starter med kredittene fra LevelSpec.
        for (i, s) in levels::LEVELS.iter().enumerate() {
            let g = Game::new_level(i);
            assert_eq!(g.credits[TEAM_PLAYER as usize], s.player_credits, "niva {}", i + 1);
            assert_eq!(g.credits[TEAM_ENEMY as usize], s.enemy_credits, "niva {}", i + 1);
        }
    }

    #[test]
    fn fiende_venter_med_angrep_til_nok_styrke() {
        // Fersk start har 3 kampenheter < forste bolge -> ingen blir aggressive.
        let mut g = Game::new();
        g.enemy_ai_decide();
        let aggro = g.units.iter().filter(|u| u.team == TEAM_ENEMY && u.aggressive).count();
        assert_eq!(aggro, 0, "skal vente med angrep");
        assert_eq!(g.enemy_waves, 0);
    }

    #[test]
    fn fiende_angriper_med_nok_styrke() {
        let mut g = Game::new();
        // Bygg reserven opp til bolgestorrelsen.
        while g.enemy_combat_units().len() < g.enemy_wave_size() {
            g.units.push(Unit::new(vec2(280.0, 230.0), TEAM_ENEMY, UnitKind::Rifleman));
        }
        g.enemy_ai_decide();
        let aggro = g.units.iter().filter(|u| u.team == TEAM_ENEMY && u.aggressive).count();
        assert!(aggro >= g.first_wave as usize, "hele bolgen skal angripe, aggro={}", aggro);
        assert_eq!(g.enemy_waves, 1, "en bolge sendt");
    }

    #[test]
    fn fiende_forsvarer_basen() {
        let mut g = Game::new();
        let hq = g.hq_pos(TEAM_ENEMY).unwrap();
        // Spiller-stridsvogn rett ved fiendens HK -> forsvar utloses.
        g.units.push(Unit::new(hq + vec2(60.0, 0.0), TEAM_PLAYER, UnitKind::Tank));
        assert!(!g.threats_near_enemy_base().is_empty(), "trussel skal oppdages");
        g.enemy_ai_decide();
        let defenders = g.units.iter().filter(|u| u.team == TEAM_ENEMY && u.aggressive).count();
        assert!(defenders > 0, "hjemmeenheter skal forsvare");
        assert_eq!(g.enemy_waves, 0, "forsvar teller ikke som angrepsbolge");
    }

    #[test]
    fn fiende_bolge_vokser_og_klampes() {
        let mut g = Game::new();
        let (fw, gr, cap) = (g.first_wave, g.wave_growth, g.wave_cap);
        g.enemy_waves = 0;
        assert_eq!(g.enemy_wave_size(), fw as usize);
        g.enemy_waves = 1;
        assert_eq!(g.enemy_wave_size(), (fw + gr) as usize);
        g.enemy_waves = 99;
        assert_eq!(g.enemy_wave_size(), cap as usize, "bolgestorrelse skal klampes");
    }

    #[test]
    fn gruppe_far_distinkte_formasjonsfelt() {
        let mut g = Game::new();
        for t in g.map.iter_mut() {
            *t = Terrain::Grass;
        }
        g.units.clear();
        // Seks enheter samlet rundt (1000,1000), alle markert.
        for k in 0..6 {
            let mut u = Unit::new(vec2(900.0 + k as f32 * 20.0, 900.0), TEAM_PLAYER, UnitKind::Tank);
            u.selected = true;
            g.units.push(u);
        }
        g.move_selected(vec2(1500.0, 1500.0));
        // Alle mal skal vaere unike (ingen to enheter sikter pa samme felt).
        let targets: Vec<Vec2> = g.units.iter().map(|u| u.target.unwrap()).collect();
        for i in 0..targets.len() {
            for j in (i + 1)..targets.len() {
                assert!(
                    targets[i].distance(targets[j]) > 1.0,
                    "enhet {} og {} fikk samme felt {:?}",
                    i, j, targets[i]
                );
            }
        }
    }
}

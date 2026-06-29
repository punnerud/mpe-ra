//! Kampanjebaner for OpenRA Rust.
//!
//! Hver `LevelSpec` beskriver ett nivå: terreng (vann/malm/fjell), baseplassering,
//! fiender (1-2 baser med ulik "stil") og vanskelighets-tall. Nivaene er
//! handlagde og skal stige gradvis i sma steg, men med god variasjon.
//!
//! `LEVELS` er kilden; `gen_map_for` bygger terrenget. Valideringstestene nederst
//! sikrer at hver bane er innenfor kartet, ikke overlapper, har malm + fiende-HQ,
//! er stiforbar (spiller -> hver fiendebase), og at vanskeligheten stiger jevnt.

use crate::{carve_blob, hash2, Terrain, MAP_H, MAP_W};

/// Fiende-"stil": former enhetsmiks + fargetint slik at baner foles ulike.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum EnemyStyle {
    Balanced, // dagens miks (rod)
    Armor,    // tank-tung (amber)
    Swarm,    // infanteri-sverm (fiolett)
}

/// En fiendebase: HQ-rute + stil.
pub struct EnemySpec {
    pub pos: (i32, i32),
    pub style: EnemyStyle,
}

/// Ett niva.
pub struct LevelSpec {
    pub player_base: (i32, i32),          // spillerens HQ-rute
    pub enemies: &'static [EnemySpec],    // 1 eller 2 fiendebaser
    pub water: &'static [(i32, i32, f32)], // (cx, cy, radius) vann-blobber
    pub ore: &'static [(i32, i32, f32)],   // malm-blobber
    pub rock_density: f32,                 // hash2-terskel (0.99=lite fjell, 0.95=mye)
    // --- vanskelighet (sma steg mellom nivaer) ---
    pub player_credits: f32,
    pub enemy_credits: f32,
    pub enemy_income: f32, // passiv kreditt/sek for fiendelaget (totalt)
    pub first_wave: u32,   // forste angrep krever sa mange kampenheter
    pub wave_growth: u32,  // okning per bolge
    pub wave_cap: u32,     // tak pa bolgestorrelse
    pub ai_tick: f32,      // sek mellom AI-beslutninger (lavere = hardere)
    pub attack_delay: f32, // sek for forste AI-beslutning
    pub enemy_power: f32,  // hp/skade-multiplikator pa fiende-enheter
}

/// Antall nivaer i kampanjen.
pub fn count() -> usize {
    LEVELS.len()
}

/// Hent et niva (klampes til gyldig omrade).
pub fn get(level: usize) -> &'static LevelSpec {
    &LEVELS[level.min(LEVELS.len() - 1)]
}

/// Bygg terrenget for et niva: fjell-drys etter `rock_density`, deretter
/// vann- og malm-blobber (samme `carve_blob` som for).
pub fn gen_map_for(spec: &LevelSpec) -> Vec<Terrain> {
    let mut map = vec![Terrain::Grass; MAP_W * MAP_H];
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            if hash2(x as i32, y as i32) > spec.rock_density {
                map[y * MAP_W + x] = Terrain::Rock;
            }
        }
    }
    for &(cx, cy, r) in spec.water {
        carve_blob(&mut map, cx, cy, r, Terrain::Water);
    }
    for &(cx, cy, r) in spec.ore {
        carve_blob(&mut map, cx, cy, r, Terrain::Ore);
    }
    map
}

// Korte aliaser for kompakte tabell-oppforinger.
use EnemyStyle::{Armor, Balanced, Swarm};

/// Kampanjebanene: 100 niva. Kart-oppsettet (baseplassering, vann, malm, stil,
/// antall fiender) er handlagd; vanskelighets-tallene folger en jevn kurve.
/// Sentinel-markorene brukes av assembler-skriptet som genererte tabellen.
// <<LEVELS_START>>
pub const LEVELS: &[LevelSpec] = &[
    // 1 -- den opprinnelige banen (gjenopprettet etter onske)
    LevelSpec {
        player_base: (55, 39),
        enemies: &[EnemySpec { pos: (8, 7), style: Balanced }],
        water: &[(12, 38, 5.0)],
        ore: &[(20, 16, 4.5), (44, 32, 5.0), (32, 24, 3.5)],
        rock_density: 0.965,
        player_credits: 1600.0,
        enemy_credits: 900.0,
        enemy_income: 7.0,
        first_wave: 5,
        wave_growth: 1,
        wave_cap: 6,
        ai_tick: 2.2,
        attack_delay: 14.0,
        enemy_power: 0.85,
    },
    // 2
    LevelSpec {
        player_base: (55, 8),
        enemies: &[EnemySpec { pos: (8, 40), style: Balanced }],
        water: &[],
        ore: &[(32, 24, 4.5), (48, 12, 4.0), (14, 36, 4.0)],
        rock_density: 0.975,
        player_credits: 1594.0,
        enemy_credits: 933.0,
        enemy_income: 7.0,
        first_wave: 5,
        wave_growth: 1,
        wave_cap: 6,
        ai_tick: 2.17,
        attack_delay: 13.7,
        enemy_power: 0.86,
    },
    // 3
    LevelSpec {
        player_base: (55, 40),
        enemies: &[EnemySpec { pos: (8, 8), style: Balanced }],
        water: &[],
        ore: &[(32, 24, 4.5), (48, 36, 4.0), (14, 12, 4.0)],
        rock_density: 0.975,
        player_credits: 1589.0,
        enemy_credits: 967.0,
        enemy_income: 8.0,
        first_wave: 5,
        wave_growth: 1,
        wave_cap: 6,
        ai_tick: 2.13,
        attack_delay: 13.3,
        enemy_power: 0.87,
    },
    // 4
    LevelSpec {
        player_base: (8, 8),
        enemies: &[EnemySpec { pos: (55, 40), style: Balanced }],
        water: &[(32, 30, 3.0)],
        ore: &[(30, 22, 4.5), (14, 12, 4.0), (50, 36, 4.0)],
        rock_density: 0.975,
        player_credits: 1583.0,
        enemy_credits: 1000.0,
        enemy_income: 8.0,
        first_wave: 5,
        wave_growth: 1,
        wave_cap: 6,
        ai_tick: 2.1,
        attack_delay: 13.0,
        enemy_power: 0.87,
    },
    // 5
    LevelSpec {
        player_base: (8, 24),
        enemies: &[EnemySpec { pos: (57, 24), style: Balanced }],
        water: &[(32, 12, 3.0)],
        ore: &[(32, 32, 4.0), (14, 24, 4.0), (50, 24, 4.0)],
        rock_density: 0.975,
        player_credits: 1578.0,
        enemy_credits: 1033.0,
        enemy_income: 9.0,
        first_wave: 5,
        wave_growth: 1,
        wave_cap: 6,
        ai_tick: 2.07,
        attack_delay: 12.7,
        enemy_power: 0.88,
    },
    // 6
    LevelSpec {
        player_base: (32, 8),
        enemies: &[EnemySpec { pos: (32, 41), style: Balanced }],
        water: &[(18, 24, 3.0)],
        ore: &[(32, 24, 4.0), (28, 12, 4.0), (36, 37, 4.0)],
        rock_density: 0.975,
        player_credits: 1572.0,
        enemy_credits: 1067.0,
        enemy_income: 9.0,
        first_wave: 5,
        wave_growth: 1,
        wave_cap: 7,
        ai_tick: 2.03,
        attack_delay: 12.3,
        enemy_power: 0.89,
    },
    // 7
    LevelSpec {
        player_base: (32, 41),
        enemies: &[EnemySpec { pos: (32, 8), style: Balanced }],
        water: &[(46, 24, 3.0)],
        ore: &[(32, 24, 4.0), (28, 37, 4.0), (36, 12, 4.0)],
        rock_density: 0.975,
        player_credits: 1567.0,
        enemy_credits: 1100.0,
        enemy_income: 10.0,
        first_wave: 5,
        wave_growth: 1,
        wave_cap: 7,
        ai_tick: 2.0,
        attack_delay: 12.0,
        enemy_power: 0.9,
    },
    // 8
    LevelSpec {
        player_base: (12, 38),
        enemies: &[EnemySpec { pos: (50, 10), style: Balanced }],
        water: &[(40, 38, 3.0)],
        ore: &[(30, 24, 4.5), (16, 32, 4.0), (46, 14, 4.0)],
        rock_density: 0.975,
        player_credits: 1561.0,
        enemy_credits: 1133.0,
        enemy_income: 10.0,
        first_wave: 5,
        wave_growth: 1,
        wave_cap: 7,
        ai_tick: 1.97,
        attack_delay: 11.7,
        enemy_power: 0.9,
    },
    // 9
    LevelSpec {
        player_base: (50, 38),
        enemies: &[EnemySpec { pos: (12, 10), style: Balanced }],
        water: &[(24, 38, 3.0)],
        ore: &[(32, 24, 4.5), (46, 32, 4.0), (16, 14, 4.0)],
        rock_density: 0.975,
        player_credits: 1556.0,
        enemy_credits: 1167.0,
        enemy_income: 11.0,
        first_wave: 5,
        wave_growth: 1,
        wave_cap: 7,
        ai_tick: 1.93,
        attack_delay: 11.3,
        enemy_power: 0.91,
    },
    // 10
    LevelSpec {
        player_base: (57, 24),
        enemies: &[EnemySpec { pos: (8, 24), style: Balanced }],
        water: &[(32, 36, 3.0)],
        ore: &[(32, 18, 4.0), (50, 24, 4.0), (14, 24, 4.0)],
        rock_density: 0.975,
        player_credits: 1550.0,
        enemy_credits: 1200.0,
        enemy_income: 11.0,
        first_wave: 5,
        wave_growth: 1,
        wave_cap: 7,
        ai_tick: 1.9,
        attack_delay: 11.0,
        enemy_power: 0.92,
    },
    // 11
    LevelSpec {
        player_base: (8, 9),
        enemies: &[EnemySpec { pos: (55, 40), style: Armor }],
        water: &[(32, 28, 4.0), (40, 16, 3.0)],
        ore: &[(20, 30, 4.0), (45, 24, 4.0), (50, 10, 3.5)],
        rock_density: 0.97,
        player_credits: 1545.0,
        enemy_credits: 1230.0,
        enemy_income: 11.0,
        first_wave: 5,
        wave_growth: 1,
        wave_cap: 7,
        ai_tick: 1.88,
        attack_delay: 10.8,
        enemy_power: 0.93,
    },
    // 12
    LevelSpec {
        player_base: (56, 40),
        enemies: &[EnemySpec { pos: (8, 8), style: Swarm }],
        water: &[(30, 20, 4.0), (38, 32, 3.0)],
        ore: &[(44, 18, 4.0), (18, 24, 4.0), (30, 38, 3.5)],
        rock_density: 0.97,
        player_credits: 1540.0,
        enemy_credits: 1260.0,
        enemy_income: 12.0,
        first_wave: 5,
        wave_growth: 1,
        wave_cap: 7,
        ai_tick: 1.86,
        attack_delay: 10.6,
        enemy_power: 0.94,
    },
    // 13
    LevelSpec {
        player_base: (56, 8),
        enemies: &[EnemySpec { pos: (9, 40), style: Balanced }],
        water: &[(30, 24, 4.0), (40, 34, 3.0)],
        ore: &[(45, 20, 4.0), (20, 30, 4.0), (34, 12, 3.5)],
        rock_density: 0.97,
        player_credits: 1535.0,
        enemy_credits: 1290.0,
        enemy_income: 12.0,
        first_wave: 5,
        wave_growth: 1,
        wave_cap: 8,
        ai_tick: 1.84,
        attack_delay: 10.4,
        enemy_power: 0.94,
    },
    // 14
    LevelSpec {
        player_base: (8, 41),
        enemies: &[EnemySpec { pos: (56, 9), style: Armor }],
        water: &[(32, 22, 4.0), (24, 32, 3.0)],
        ore: &[(18, 28, 4.0), (45, 18, 4.0), (40, 36, 3.5)],
        rock_density: 0.97,
        player_credits: 1530.0,
        enemy_credits: 1320.0,
        enemy_income: 12.0,
        first_wave: 5,
        wave_growth: 1,
        wave_cap: 8,
        ai_tick: 1.82,
        attack_delay: 10.2,
        enemy_power: 0.95,
    },
    // 15
    LevelSpec {
        player_base: (6, 24),
        enemies: &[EnemySpec { pos: (57, 24), style: Balanced }],
        water: &[(32, 14, 4.0), (32, 34, 4.0)],
        ore: &[(20, 30, 4.0), (44, 18, 4.0), (32, 24, 3.5)],
        rock_density: 0.97,
        player_credits: 1525.0,
        enemy_credits: 1350.0,
        enemy_income: 12.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 8,
        ai_tick: 1.8,
        attack_delay: 10.0,
        enemy_power: 0.96,
    },
    // 16
    LevelSpec {
        player_base: (32, 7),
        enemies: &[EnemySpec { pos: (32, 41), style: Swarm }],
        water: &[(20, 24, 4.0), (44, 24, 4.0)],
        ore: &[(32, 24, 3.5), (14, 16, 4.0), (50, 32, 4.0)],
        rock_density: 0.97,
        player_credits: 1520.0,
        enemy_credits: 1380.0,
        enemy_income: 13.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 8,
        ai_tick: 1.78,
        attack_delay: 9.8,
        enemy_power: 0.97,
    },
    // 17
    LevelSpec {
        player_base: (12, 8),
        enemies: &[EnemySpec { pos: (52, 38), style: Balanced }],
        water: &[(34, 24, 4.0), (26, 16, 3.0)],
        ore: &[(24, 28, 4.0), (46, 20, 4.0), (40, 34, 3.5)],
        rock_density: 0.97,
        player_credits: 1515.0,
        enemy_credits: 1410.0,
        enemy_income: 13.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 8,
        ai_tick: 1.76,
        attack_delay: 9.6,
        enemy_power: 0.98,
    },
    // 18
    LevelSpec {
        player_base: (54, 41),
        enemies: &[EnemySpec { pos: (10, 10), style: Armor }],
        water: &[(30, 24, 4.0), (40, 16, 3.0)],
        ore: &[(22, 18, 4.0), (44, 30, 4.0), (34, 36, 3.5)],
        rock_density: 0.97,
        player_credits: 1510.0,
        enemy_credits: 1440.0,
        enemy_income: 13.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 9,
        ai_tick: 1.74,
        attack_delay: 9.4,
        enemy_power: 0.98,
    },
    // 19
    LevelSpec {
        player_base: (50, 8),
        enemies: &[EnemySpec { pos: (10, 38), style: Swarm }],
        water: &[(30, 22, 4.0), (38, 32, 3.0)],
        ore: &[(40, 18, 4.0), (20, 28, 4.0), (30, 12, 3.5)],
        rock_density: 0.97,
        player_credits: 1505.0,
        enemy_credits: 1470.0,
        enemy_income: 14.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 9,
        ai_tick: 1.72,
        attack_delay: 9.2,
        enemy_power: 0.99,
    },
    // 20
    LevelSpec {
        player_base: (8, 38),
        enemies: &[EnemySpec { pos: (54, 10), style: Balanced }],
        water: &[(30, 24, 4.0), (22, 16, 3.0)],
        ore: &[(20, 28, 4.0), (44, 18, 4.0), (38, 32, 3.5)],
        rock_density: 0.97,
        player_credits: 1500.0,
        enemy_credits: 1500.0,
        enemy_income: 14.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 9,
        ai_tick: 1.7,
        attack_delay: 9.0,
        enemy_power: 1.0,
    },
    // 21
    LevelSpec {
        player_base: (8, 8),
        enemies: &[EnemySpec { pos: (56, 40), style: Swarm }],
        water: &[(24, 32, 4.0), (42, 16, 3.0)],
        ore: &[(18, 18, 4.0), (46, 30, 4.0)],
        rock_density: 0.97,
        player_credits: 1495.0,
        enemy_credits: 1535.0,
        enemy_income: 14.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 9,
        ai_tick: 1.68,
        attack_delay: 8.8,
        enemy_power: 1.01,
    },
    // 22
    LevelSpec {
        player_base: (56, 8),
        enemies: &[EnemySpec { pos: (8, 40), style: Armor }],
        water: &[(40, 32, 4.0), (20, 16, 3.0)],
        ore: &[(46, 18, 4.0), (18, 30, 4.0)],
        rock_density: 0.97,
        player_credits: 1490.0,
        enemy_credits: 1570.0,
        enemy_income: 15.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 9,
        ai_tick: 1.66,
        attack_delay: 8.7,
        enemy_power: 1.01,
    },
    // 23
    LevelSpec {
        player_base: (8, 40),
        enemies: &[EnemySpec { pos: (56, 8), style: Balanced }],
        water: &[(24, 16, 4.0), (42, 30, 3.0)],
        ore: &[(18, 30, 4.0), (46, 18, 4.0)],
        rock_density: 0.97,
        player_credits: 1485.0,
        enemy_credits: 1605.0,
        enemy_income: 15.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 10,
        ai_tick: 1.64,
        attack_delay: 8.6,
        enemy_power: 1.02,
    },
    // 24
    LevelSpec {
        player_base: (32, 7),
        enemies: &[EnemySpec { pos: (32, 41), style: Swarm }],
        water: &[(16, 24, 4.0), (48, 24, 4.0)],
        ore: &[(20, 24, 4.0), (44, 24, 4.0)],
        rock_density: 0.97,
        player_credits: 1480.0,
        enemy_credits: 1640.0,
        enemy_income: 16.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 10,
        ai_tick: 1.62,
        attack_delay: 8.4,
        enemy_power: 1.02,
    },
    // 25
    LevelSpec {
        player_base: (6, 24),
        enemies: &[EnemySpec { pos: (57, 24), style: Armor }],
        water: &[(32, 12, 4.0), (32, 36, 4.0)],
        ore: &[(24, 18, 4.0), (40, 30, 4.0)],
        rock_density: 0.97,
        player_credits: 1475.0,
        enemy_credits: 1675.0,
        enemy_income: 16.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 10,
        ai_tick: 1.6,
        attack_delay: 8.2,
        enemy_power: 1.03,
    },
    // 26
    LevelSpec {
        player_base: (56, 40),
        enemies: &[EnemySpec { pos: (8, 8), style: Balanced }],
        water: &[(40, 16, 4.0), (22, 32, 3.0)],
        ore: &[(44, 30, 4.0), (20, 18, 4.0)],
        rock_density: 0.97,
        player_credits: 1470.0,
        enemy_credits: 1710.0,
        enemy_income: 16.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 10,
        ai_tick: 1.58,
        attack_delay: 8.1,
        enemy_power: 1.04,
    },
    // 27
    LevelSpec {
        player_base: (20, 8),
        enemies: &[EnemySpec { pos: (50, 41), style: Swarm }],
        water: &[(30, 30, 4.0), (44, 18, 3.0)],
        ore: &[(24, 16, 4.0), (46, 32, 4.0)],
        rock_density: 0.97,
        player_credits: 1465.0,
        enemy_credits: 1745.0,
        enemy_income: 17.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 10,
        ai_tick: 1.56,
        attack_delay: 8.0,
        enemy_power: 1.04,
    },
    // 28
    LevelSpec {
        player_base: (50, 10),
        enemies: &[EnemySpec { pos: (12, 38), style: Armor }],
        water: &[(28, 20, 4.0), (36, 32, 3.0)],
        ore: &[(42, 18, 4.0), (20, 30, 4.0)],
        rock_density: 0.97,
        player_credits: 1460.0,
        enemy_credits: 1780.0,
        enemy_income: 17.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 11,
        ai_tick: 1.54,
        attack_delay: 7.8,
        enemy_power: 1.05,
    },
    // 29
    LevelSpec {
        player_base: (10, 38),
        enemies: &[EnemySpec { pos: (54, 12), style: Balanced }],
        water: &[(28, 28, 4.0), (40, 18, 3.0)],
        ore: &[(20, 30, 4.0), (46, 18, 4.0)],
        rock_density: 0.97,
        player_credits: 1455.0,
        enemy_credits: 1815.0,
        enemy_income: 18.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 11,
        ai_tick: 1.52,
        attack_delay: 7.7,
        enemy_power: 1.05,
    },
    // 30
    LevelSpec {
        player_base: (32, 41),
        enemies: &[EnemySpec { pos: (32, 7), style: Swarm }],
        water: &[(16, 24, 4.0), (48, 24, 4.0)],
        ore: &[(20, 24, 4.0), (44, 24, 4.0)],
        rock_density: 0.97,
        player_credits: 1450.0,
        enemy_credits: 1850.0,
        enemy_income: 18.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 11,
        ai_tick: 1.5,
        attack_delay: 7.5,
        enemy_power: 1.06,
    },
    // 31
    LevelSpec {
        player_base: (8, 40),
        enemies: &[EnemySpec { pos: (55, 8), style: Balanced }],
        water: &[(30, 30, 4.0), (40, 18, 3.0)],
        ore: &[(20, 30, 4.0), (45, 18, 4.0)],
        rock_density: 0.965,
        player_credits: 1445.0,
        enemy_credits: 1875.0,
        enemy_income: 18.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 11,
        ai_tick: 1.49,
        attack_delay: 7.4,
        enemy_power: 1.06,
    },
    // 32
    LevelSpec {
        player_base: (56, 8),
        enemies: &[EnemySpec { pos: (8, 40), style: Swarm }],
        water: &[(28, 18, 4.0), (38, 30, 3.0)],
        ore: &[(44, 18, 4.0), (20, 30, 4.0)],
        rock_density: 0.965,
        player_credits: 1440.0,
        enemy_credits: 1900.0,
        enemy_income: 19.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 11,
        ai_tick: 1.47,
        attack_delay: 7.3,
        enemy_power: 1.07,
    },
    // 33
    LevelSpec {
        player_base: (8, 8),
        enemies: &[EnemySpec { pos: (55, 40), style: Armor }],
        water: &[(30, 30, 4.0), (40, 16, 3.0)],
        ore: &[(20, 20, 4.0), (44, 30, 4.0)],
        rock_density: 0.965,
        player_credits: 1435.0,
        enemy_credits: 1925.0,
        enemy_income: 19.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 12,
        ai_tick: 1.46,
        attack_delay: 7.2,
        enemy_power: 1.07,
    },
    // 34
    LevelSpec {
        player_base: (56, 40),
        enemies: &[EnemySpec { pos: (8, 10), style: Balanced }],
        water: &[(30, 20, 4.0), (40, 30, 3.0)],
        ore: &[(44, 28, 4.0), (20, 20, 4.0)],
        rock_density: 0.965,
        player_credits: 1430.0,
        enemy_credits: 1950.0,
        enemy_income: 19.0,
        first_wave: 5,
        wave_growth: 2,
        wave_cap: 12,
        ai_tick: 1.44,
        attack_delay: 7.1,
        enemy_power: 1.08,
    },
    // 35
    LevelSpec {
        player_base: (8, 24),
        enemies: &[EnemySpec { pos: (50, 8), style: Armor }, EnemySpec { pos: (50, 40), style: Swarm }],
        water: &[(33, 14, 3.0), (33, 34, 3.0)],
        ore: &[(24, 16, 4.0), (24, 32, 4.0)],
        rock_density: 0.965,
        player_credits: 1425.0,
        enemy_credits: 1975.0,
        enemy_income: 20.0,
        first_wave: 6,
        wave_growth: 2,
        wave_cap: 12,
        ai_tick: 1.43,
        attack_delay: 7.0,
        enemy_power: 1.08,
    },
    // 36
    LevelSpec {
        player_base: (56, 24),
        enemies: &[EnemySpec { pos: (8, 24), style: Swarm }],
        water: &[(32, 12, 4.0), (32, 36, 4.0)],
        ore: &[(40, 16, 4.0), (24, 32, 4.0)],
        rock_density: 0.965,
        player_credits: 1420.0,
        enemy_credits: 2000.0,
        enemy_income: 20.0,
        first_wave: 6,
        wave_growth: 2,
        wave_cap: 12,
        ai_tick: 1.41,
        attack_delay: 6.9,
        enemy_power: 1.08,
    },
    // 37
    LevelSpec {
        player_base: (32, 40),
        enemies: &[EnemySpec { pos: (32, 8), style: Armor }],
        water: &[(18, 24, 4.0), (46, 24, 4.0)],
        ore: &[(24, 24, 4.0), (40, 24, 4.0)],
        rock_density: 0.965,
        player_credits: 1415.0,
        enemy_credits: 2025.0,
        enemy_income: 20.0,
        first_wave: 6,
        wave_growth: 2,
        wave_cap: 12,
        ai_tick: 1.4,
        attack_delay: 6.8,
        enemy_power: 1.09,
    },
    // 38
    LevelSpec {
        player_base: (32, 8),
        enemies: &[EnemySpec { pos: (56, 40), style: Balanced }],
        water: &[(24, 28, 4.0), (44, 20, 3.0)],
        ore: &[(24, 18, 4.0), (44, 30, 4.0)],
        rock_density: 0.965,
        player_credits: 1410.0,
        enemy_credits: 2050.0,
        enemy_income: 20.0,
        first_wave: 6,
        wave_growth: 2,
        wave_cap: 13,
        ai_tick: 1.38,
        attack_delay: 6.7,
        enemy_power: 1.09,
    },
    // 39
    LevelSpec {
        player_base: (10, 38),
        enemies: &[EnemySpec { pos: (55, 10), style: Armor }, EnemySpec { pos: (38, 8), style: Swarm }],
        water: &[(28, 28, 4.0), (40, 22, 3.0)],
        ore: &[(24, 22, 4.0), (46, 16, 4.0)],
        rock_density: 0.965,
        player_credits: 1405.0,
        enemy_credits: 2075.0,
        enemy_income: 21.0,
        first_wave: 6,
        wave_growth: 2,
        wave_cap: 13,
        ai_tick: 1.36,
        attack_delay: 6.6,
        enemy_power: 1.1,
    },
    // 40
    LevelSpec {
        player_base: (54, 10),
        enemies: &[EnemySpec { pos: (8, 40), style: Armor }],
        water: &[(30, 20, 4.0), (38, 30, 3.0)],
        ore: &[(40, 22, 4.0), (22, 32, 4.0)],
        rock_density: 0.965,
        player_credits: 1400.0,
        enemy_credits: 2100.0,
        enemy_income: 21.0,
        first_wave: 6,
        wave_growth: 2,
        wave_cap: 13,
        ai_tick: 1.35,
        attack_delay: 6.5,
        enemy_power: 1.1,
    },
    // 41
    LevelSpec {
        player_base: (8, 8),
        enemies: &[EnemySpec { pos: (50, 40), style: Balanced }],
        water: &[(30, 30, 4.0), (40, 15, 3.0)],
        ore: &[(20, 35, 4.0), (45, 12, 4.0)],
        rock_density: 0.965,
        player_credits: 1395.0,
        enemy_credits: 2130.0,
        enemy_income: 21.0,
        first_wave: 6,
        wave_growth: 2,
        wave_cap: 13,
        ai_tick: 1.34,
        attack_delay: 6.4,
        enemy_power: 1.1,
    },
    // 42
    LevelSpec {
        player_base: (55, 40),
        enemies: &[EnemySpec { pos: (10, 8), style: Swarm }],
        water: &[(32, 24, 4.0), (40, 12, 3.0)],
        ore: &[(20, 30, 4.0), (48, 25, 4.0)],
        rock_density: 0.965,
        player_credits: 1390.0,
        enemy_credits: 2160.0,
        enemy_income: 22.0,
        first_wave: 6,
        wave_growth: 2,
        wave_cap: 13,
        ai_tick: 1.33,
        attack_delay: 6.3,
        enemy_power: 1.11,
    },
    // 43
    LevelSpec {
        player_base: (55, 8),
        enemies: &[EnemySpec { pos: (10, 40), style: Armor }, EnemySpec { pos: (12, 18), style: Swarm }],
        water: &[(35, 25, 4.0), (40, 40, 3.0)],
        ore: &[(30, 12, 4.0), (25, 35, 4.0)],
        rock_density: 0.965,
        player_credits: 1385.0,
        enemy_credits: 2190.0,
        enemy_income: 22.0,
        first_wave: 6,
        wave_growth: 2,
        wave_cap: 14,
        ai_tick: 1.32,
        attack_delay: 6.2,
        enemy_power: 1.11,
    },
    // 44
    LevelSpec {
        player_base: (8, 40),
        enemies: &[EnemySpec { pos: (52, 8), style: Armor }],
        water: &[(30, 24, 4.0), (40, 35, 3.0)],
        ore: &[(18, 18, 4.0), (45, 30, 4.0)],
        rock_density: 0.965,
        player_credits: 1380.0,
        enemy_credits: 2220.0,
        enemy_income: 22.0,
        first_wave: 6,
        wave_growth: 2,
        wave_cap: 14,
        ai_tick: 1.31,
        attack_delay: 6.1,
        enemy_power: 1.12,
    },
    // 45
    LevelSpec {
        player_base: (32, 8),
        enemies: &[EnemySpec { pos: (32, 40), style: Swarm }],
        water: &[(16, 24, 4.0), (48, 24, 4.0)],
        ore: &[(20, 12, 4.0), (44, 36, 4.0)],
        rock_density: 0.965,
        player_credits: 1375.0,
        enemy_credits: 2250.0,
        enemy_income: 22.0,
        first_wave: 6,
        wave_growth: 2,
        wave_cap: 14,
        ai_tick: 1.3,
        attack_delay: 6.0,
        enemy_power: 1.12,
    },
    // 46
    LevelSpec {
        player_base: (8, 24),
        enemies: &[EnemySpec { pos: (52, 24), style: Balanced }],
        water: &[(30, 12, 4.0), (30, 36, 4.0)],
        ore: &[(20, 35, 4.0), (42, 12, 4.0)],
        rock_density: 0.965,
        player_credits: 1370.0,
        enemy_credits: 2280.0,
        enemy_income: 23.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 14,
        ai_tick: 1.29,
        attack_delay: 5.9,
        enemy_power: 1.13,
    },
    // 47
    LevelSpec {
        player_base: (32, 40),
        enemies: &[EnemySpec { pos: (10, 8), style: Swarm }, EnemySpec { pos: (54, 8), style: Armor }],
        water: &[(32, 20, 4.0), (20, 30, 3.0)],
        ore: &[(42, 28, 4.0), (25, 15, 4.0)],
        rock_density: 0.965,
        player_credits: 1365.0,
        enemy_credits: 2310.0,
        enemy_income: 23.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 14,
        ai_tick: 1.28,
        attack_delay: 5.8,
        enemy_power: 1.14,
    },
    // 48
    LevelSpec {
        player_base: (54, 9),
        enemies: &[EnemySpec { pos: (9, 40), style: Balanced }],
        water: &[(30, 24, 4.0), (40, 35, 3.0)],
        ore: &[(20, 18, 4.0), (45, 30, 4.0)],
        rock_density: 0.965,
        player_credits: 1360.0,
        enemy_credits: 2340.0,
        enemy_income: 23.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 15,
        ai_tick: 1.27,
        attack_delay: 5.7,
        enemy_power: 1.14,
    },
    // 49
    LevelSpec {
        player_base: (54, 40),
        enemies: &[EnemySpec { pos: (28, 8), style: Swarm }],
        water: &[(35, 24, 4.0), (20, 30, 3.0)],
        ore: &[(42, 15, 4.0), (18, 38, 4.0)],
        rock_density: 0.965,
        player_credits: 1355.0,
        enemy_credits: 2370.0,
        enemy_income: 24.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 15,
        ai_tick: 1.26,
        attack_delay: 5.6,
        enemy_power: 1.15,
    },
    // 50
    LevelSpec {
        player_base: (6, 6),
        enemies: &[EnemySpec { pos: (57, 41), style: Armor }],
        water: &[(32, 24, 5.0), (44, 14, 4.0), (20, 34, 4.0)],
        ore: &[(42, 38, 5.0), (14, 12, 5.0)],
        rock_density: 0.965,
        player_credits: 1350.0,
        enemy_credits: 2400.0,
        enemy_income: 24.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 15,
        ai_tick: 1.25,
        attack_delay: 5.5,
        enemy_power: 1.15,
    },
    // 51
    LevelSpec {
        player_base: (8, 40),
        enemies: &[EnemySpec { pos: (55, 8), style: Armor }, EnemySpec { pos: (55, 40), style: Swarm }],
        water: &[(30, 24, 5.0), (20, 12, 4.0), (40, 30, 4.0)],
        ore: &[(25, 38, 4.0), (45, 18, 4.0)],
        rock_density: 0.962,
        player_credits: 1345.0,
        enemy_credits: 2430.0,
        enemy_income: 24.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 15,
        ai_tick: 1.24,
        attack_delay: 5.5,
        enemy_power: 1.15,
    },
    // 52
    LevelSpec {
        player_base: (56, 8),
        enemies: &[EnemySpec { pos: (8, 40), style: Balanced }],
        water: &[(30, 24, 5.0), (40, 16, 4.0)],
        ore: &[(20, 20, 4.0), (45, 35, 4.0)],
        rock_density: 0.962,
        player_credits: 1340.0,
        enemy_credits: 2460.0,
        enemy_income: 25.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 16,
        ai_tick: 1.23,
        attack_delay: 5.4,
        enemy_power: 1.16,
    },
    // 53
    LevelSpec {
        player_base: (8, 8),
        enemies: &[EnemySpec { pos: (56, 40), style: Armor }],
        water: &[(32, 24, 5.0), (24, 36, 4.0)],
        ore: &[(40, 14, 4.0), (18, 30, 4.0)],
        rock_density: 0.962,
        player_credits: 1335.0,
        enemy_credits: 2490.0,
        enemy_income: 25.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 16,
        ai_tick: 1.22,
        attack_delay: 5.3,
        enemy_power: 1.17,
    },
    // 54
    LevelSpec {
        player_base: (56, 40),
        enemies: &[EnemySpec { pos: (8, 8), style: Swarm }],
        water: &[(30, 22, 5.0), (42, 32, 4.0)],
        ore: &[(20, 18, 4.0), (45, 25, 4.0)],
        rock_density: 0.962,
        player_credits: 1330.0,
        enemy_credits: 2520.0,
        enemy_income: 25.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 16,
        ai_tick: 1.21,
        attack_delay: 5.3,
        enemy_power: 1.17,
    },
    // 55
    LevelSpec {
        player_base: (7, 24),
        enemies: &[EnemySpec { pos: (52, 9), style: Armor }, EnemySpec { pos: (52, 40), style: Swarm }],
        water: &[(30, 16, 5.0), (30, 34, 5.0)],
        ore: &[(20, 38, 4.0), (45, 16, 4.0)],
        rock_density: 0.962,
        player_credits: 1325.0,
        enemy_credits: 2550.0,
        enemy_income: 26.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 16,
        ai_tick: 1.2,
        attack_delay: 5.2,
        enemy_power: 1.17,
    },
    // 56
    LevelSpec {
        player_base: (32, 7),
        enemies: &[EnemySpec { pos: (30, 41), style: Balanced }],
        water: &[(15, 24, 5.0), (48, 24, 5.0)],
        ore: &[(20, 16, 4.0), (44, 34, 4.0)],
        rock_density: 0.962,
        player_credits: 1320.0,
        enemy_credits: 2580.0,
        enemy_income: 26.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 17,
        ai_tick: 1.19,
        attack_delay: 5.2,
        enemy_power: 1.18,
    },
    // 57
    LevelSpec {
        player_base: (10, 41),
        enemies: &[EnemySpec { pos: (54, 9), style: Armor }],
        water: &[(32, 24, 5.0), (40, 38, 4.0), (20, 14, 4.0)],
        ore: &[(25, 35, 4.0), (48, 20, 4.0)],
        rock_density: 0.962,
        player_credits: 1315.0,
        enemy_credits: 2610.0,
        enemy_income: 26.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 17,
        ai_tick: 1.18,
        attack_delay: 5.2,
        enemy_power: 1.19,
    },
    // 58
    LevelSpec {
        player_base: (57, 24),
        enemies: &[EnemySpec { pos: (9, 14), style: Swarm }],
        water: &[(30, 16, 5.0), (34, 36, 4.0)],
        ore: &[(22, 30, 4.0), (44, 12, 4.0)],
        rock_density: 0.962,
        player_credits: 1310.0,
        enemy_credits: 2640.0,
        enemy_income: 26.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 17,
        ai_tick: 1.17,
        attack_delay: 5.1,
        enemy_power: 1.19,
    },
    // 59
    LevelSpec {
        player_base: (32, 41),
        enemies: &[EnemySpec { pos: (10, 9), style: Swarm }, EnemySpec { pos: (54, 9), style: Armor }],
        water: &[(20, 24, 5.0), (44, 24, 5.0)],
        ore: &[(16, 14, 4.0), (48, 14, 4.0)],
        rock_density: 0.962,
        player_credits: 1305.0,
        enemy_credits: 2670.0,
        enemy_income: 27.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 18,
        ai_tick: 1.16,
        attack_delay: 5.0,
        enemy_power: 1.19,
    },
    // 60
    LevelSpec {
        player_base: (54, 9),
        enemies: &[EnemySpec { pos: (10, 40), style: Balanced }],
        water: &[(30, 22, 5.0), (40, 34, 4.0), (20, 30, 4.0)],
        ore: &[(25, 38, 4.0), (45, 20, 4.0)],
        rock_density: 0.962,
        player_credits: 1300.0,
        enemy_credits: 2700.0,
        enemy_income: 27.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 18,
        ai_tick: 1.15,
        attack_delay: 5.0,
        enemy_power: 1.2,
    },
    // 61
    LevelSpec {
        player_base: (8, 40),
        enemies: &[EnemySpec { pos: (55, 8), style: Armor }, EnemySpec { pos: (12, 8), style: Swarm }],
        water: &[(32, 24, 5.0), (40, 38, 4.0), (24, 16, 5.0)],
        ore: &[(44, 30, 4.0)],
        rock_density: 0.958,
        player_credits: 1295.0,
        enemy_credits: 2730.0,
        enemy_income: 27.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 18,
        ai_tick: 1.14,
        attack_delay: 4.9,
        enemy_power: 1.21,
    },
    // 62
    LevelSpec {
        player_base: (7, 7),
        enemies: &[EnemySpec { pos: (56, 41), style: Armor }],
        water: &[(30, 24, 5.0), (20, 38, 4.0), (45, 14, 5.0)],
        ore: &[(40, 30, 4.0)],
        rock_density: 0.958,
        player_credits: 1290.0,
        enemy_credits: 2760.0,
        enemy_income: 28.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 19,
        ai_tick: 1.13,
        attack_delay: 4.9,
        enemy_power: 1.21,
    },
    // 63
    LevelSpec {
        player_base: (56, 8),
        enemies: &[EnemySpec { pos: (8, 40), style: Swarm }, EnemySpec { pos: (50, 41), style: Armor }],
        water: &[(30, 24, 5.0), (28, 40, 4.0), (40, 20, 5.0)],
        ore: &[(20, 18, 4.0)],
        rock_density: 0.958,
        player_credits: 1285.0,
        enemy_credits: 2790.0,
        enemy_income: 28.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 19,
        ai_tick: 1.12,
        attack_delay: 4.8,
        enemy_power: 1.21,
    },
    // 64
    LevelSpec {
        player_base: (57, 41),
        enemies: &[EnemySpec { pos: (7, 7), style: Swarm }],
        water: &[(32, 24, 5.0), (40, 38, 4.0), (18, 20, 5.0)],
        ore: &[(25, 30, 4.0)],
        rock_density: 0.958,
        player_credits: 1280.0,
        enemy_credits: 2820.0,
        enemy_income: 28.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 19,
        ai_tick: 1.11,
        attack_delay: 4.7,
        enemy_power: 1.22,
    },
    // 65
    LevelSpec {
        player_base: (7, 24),
        enemies: &[EnemySpec { pos: (54, 9), style: Armor }, EnemySpec { pos: (54, 40), style: Swarm }],
        water: &[(32, 24, 5.0), (30, 10, 4.0), (30, 38, 4.0)],
        ore: &[(40, 24, 4.0)],
        rock_density: 0.958,
        player_credits: 1275.0,
        enemy_credits: 2850.0,
        enemy_income: 28.0,
        first_wave: 6,
        wave_growth: 3,
        wave_cap: 20,
        ai_tick: 1.1,
        attack_delay: 4.7,
        enemy_power: 1.23,
    },
    // 66
    LevelSpec {
        player_base: (32, 7),
        enemies: &[EnemySpec { pos: (32, 41), style: Armor }],
        water: &[(18, 24, 5.0), (46, 24, 5.0), (10, 38, 4.0)],
        ore: &[(32, 24, 4.0)],
        rock_density: 0.958,
        player_credits: 1270.0,
        enemy_credits: 2880.0,
        enemy_income: 29.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 20,
        ai_tick: 1.09,
        attack_delay: 4.6,
        enemy_power: 1.23,
    },
    // 67
    LevelSpec {
        player_base: (9, 41),
        enemies: &[EnemySpec { pos: (10, 8), style: Swarm }, EnemySpec { pos: (54, 12), style: Armor }],
        water: &[(30, 26, 5.0), (40, 40, 4.0), (34, 12, 4.0)],
        ore: &[(24, 20, 4.0)],
        rock_density: 0.958,
        player_credits: 1265.0,
        enemy_credits: 2910.0,
        enemy_income: 29.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 20,
        ai_tick: 1.08,
        attack_delay: 4.5,
        enemy_power: 1.23,
    },
    // 68
    LevelSpec {
        player_base: (57, 24),
        enemies: &[EnemySpec { pos: (7, 24), style: Armor }],
        water: &[(32, 12, 5.0), (32, 36, 5.0), (20, 24, 4.0)],
        ore: &[(42, 34, 4.0)],
        rock_density: 0.958,
        player_credits: 1260.0,
        enemy_credits: 2940.0,
        enemy_income: 29.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 20,
        ai_tick: 1.07,
        attack_delay: 4.4,
        enemy_power: 1.24,
    },
    // 69
    LevelSpec {
        player_base: (20, 7),
        enemies: &[EnemySpec { pos: (9, 41), style: Armor }, EnemySpec { pos: (55, 40), style: Swarm }],
        water: &[(32, 22, 5.0), (40, 40, 4.0), (22, 30, 5.0)],
        ore: &[(48, 22, 4.0)],
        rock_density: 0.958,
        player_credits: 1255.0,
        enemy_credits: 2970.0,
        enemy_income: 30.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 21,
        ai_tick: 1.06,
        attack_delay: 4.4,
        enemy_power: 1.25,
    },
    // 70
    LevelSpec {
        player_base: (32, 41),
        enemies: &[EnemySpec { pos: (55, 8), style: Swarm }],
        water: &[(16, 24, 5.0), (48, 28, 5.0), (24, 12, 4.0)],
        ore: &[(44, 20, 4.0)],
        rock_density: 0.958,
        player_credits: 1250.0,
        enemy_credits: 3000.0,
        enemy_income: 30.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 21,
        ai_tick: 1.05,
        attack_delay: 4.3,
        enemy_power: 1.25,
    },
    // 71
    LevelSpec {
        player_base: (8, 40),
        enemies: &[EnemySpec { pos: (56, 10), style: Swarm }, EnemySpec { pos: (38, 8), style: Armor }],
        water: &[(25, 26, 5.0), (46, 30, 5.0), (20, 15, 4.0)],
        ore: &[(15, 24, 4.0)],
        rock_density: 0.958,
        player_credits: 1240.0,
        enemy_credits: 3040.0,
        enemy_income: 30.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 21,
        ai_tick: 1.04,
        attack_delay: 4.2,
        enemy_power: 1.25,
    },
    // 72
    LevelSpec {
        player_base: (10, 8),
        enemies: &[EnemySpec { pos: (54, 40), style: Armor }],
        water: &[(30, 22, 6.0), (42, 12, 5.0), (20, 34, 4.0)],
        ore: &[(34, 38, 4.0)],
        rock_density: 0.958,
        player_credits: 1230.0,
        enemy_credits: 3080.0,
        enemy_income: 31.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 22,
        ai_tick: 1.03,
        attack_delay: 4.2,
        enemy_power: 1.26,
    },
    // 73
    LevelSpec {
        player_base: (56, 40),
        enemies: &[EnemySpec { pos: (10, 10), style: Swarm }, EnemySpec { pos: (10, 38), style: Armor }],
        water: &[(34, 24, 6.0), (40, 12, 5.0), (28, 38, 4.0)],
        ore: &[(44, 28, 4.0)],
        rock_density: 0.958,
        player_credits: 1220.0,
        enemy_credits: 3120.0,
        enemy_income: 31.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 22,
        ai_tick: 1.02,
        attack_delay: 4.1,
        enemy_power: 1.27,
    },
    // 74
    LevelSpec {
        player_base: (56, 8),
        enemies: &[EnemySpec { pos: (10, 40), style: Swarm }],
        water: &[(32, 24, 6.0), (24, 14, 5.0), (40, 34, 4.0)],
        ore: &[(30, 36, 4.0)],
        rock_density: 0.958,
        player_credits: 1210.0,
        enemy_credits: 3160.0,
        enemy_income: 32.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 23,
        ai_tick: 1.01,
        attack_delay: 4.1,
        enemy_power: 1.27,
    },
    // 75
    LevelSpec {
        player_base: (8, 24),
        enemies: &[EnemySpec { pos: (56, 10), style: Armor }, EnemySpec { pos: (56, 40), style: Swarm }],
        water: &[(30, 24, 5.0), (40, 14, 5.0), (40, 36, 5.0)],
        ore: &[(22, 24, 4.0)],
        rock_density: 0.958,
        player_credits: 1200.0,
        enemy_credits: 3200.0,
        enemy_income: 32.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 23,
        ai_tick: 1.0,
        attack_delay: 4.0,
        enemy_power: 1.27,
    },
    // 76
    LevelSpec {
        player_base: (32, 41),
        enemies: &[EnemySpec { pos: (32, 8), style: Balanced }],
        water: &[(18, 24, 6.0), (48, 24, 6.0), (20, 38, 4.0)],
        ore: &[(46, 38, 4.0)],
        rock_density: 0.958,
        player_credits: 1190.0,
        enemy_credits: 3240.0,
        enemy_income: 32.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 23,
        ai_tick: 0.99,
        attack_delay: 4.0,
        enemy_power: 1.28,
    },
    // 77
    LevelSpec {
        player_base: (8, 8),
        enemies: &[EnemySpec { pos: (56, 28), style: Armor }, EnemySpec { pos: (40, 41), style: Balanced }],
        water: &[(28, 20, 5.0), (44, 14, 5.0), (24, 36, 4.0)],
        ore: &[(18, 26, 4.0)],
        rock_density: 0.958,
        player_credits: 1180.0,
        enemy_credits: 3280.0,
        enemy_income: 33.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 24,
        ai_tick: 0.98,
        attack_delay: 3.9,
        enemy_power: 1.29,
    },
    // 78
    LevelSpec {
        player_base: (54, 40),
        enemies: &[EnemySpec { pos: (8, 12), style: Armor }],
        water: &[(30, 26, 6.0), (24, 16, 5.0), (40, 32, 4.0)],
        ore: &[(34, 22, 4.0)],
        rock_density: 0.958,
        player_credits: 1170.0,
        enemy_credits: 3320.0,
        enemy_income: 33.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 24,
        ai_tick: 0.97,
        attack_delay: 3.9,
        enemy_power: 1.29,
    },
    // 79
    LevelSpec {
        player_base: (32, 8),
        enemies: &[EnemySpec { pos: (8, 40), style: Swarm }, EnemySpec { pos: (56, 40), style: Balanced }],
        water: &[(20, 26, 5.0), (44, 26, 5.0), (32, 32, 4.0)],
        ore: &[(32, 20, 4.0)],
        rock_density: 0.958,
        player_credits: 1160.0,
        enemy_credits: 3360.0,
        enemy_income: 34.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 25,
        ai_tick: 0.96,
        attack_delay: 3.8,
        enemy_power: 1.29,
    },
    // 80
    LevelSpec {
        player_base: (10, 40),
        enemies: &[EnemySpec { pos: (54, 10), style: Swarm }],
        water: &[(32, 24, 6.0), (40, 16, 5.0), (22, 30, 4.0)],
        ore: &[(30, 34, 4.0)],
        rock_density: 0.958,
        player_credits: 1150.0,
        enemy_credits: 3400.0,
        enemy_income: 34.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 25,
        ai_tick: 0.95,
        attack_delay: 3.8,
        enemy_power: 1.3,
    },
    // 81
    LevelSpec {
        player_base: (8, 9),
        enemies: &[EnemySpec { pos: (55, 12), style: Armor }, EnemySpec { pos: (50, 40), style: Swarm }],
        water: &[(30, 8, 5.0), (24, 30, 5.0), (42, 24, 4.0), (14, 40, 4.0)],
        ore: &[(38, 8, 4.0), (16, 22, 4.0)],
        rock_density: 0.955,
        player_credits: 1143.0,
        enemy_credits: 3430.0,
        enemy_income: 34.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 26,
        ai_tick: 0.94,
        attack_delay: 3.8,
        enemy_power: 1.31,
    },
    // 82
    LevelSpec {
        player_base: (9, 40),
        enemies: &[EnemySpec { pos: (54, 8), style: Armor }],
        water: &[(28, 22, 6.0), (44, 34, 5.0), (20, 12, 4.0)],
        ore: &[(40, 40, 4.5)],
        rock_density: 0.955,
        player_credits: 1136.0,
        enemy_credits: 3460.0,
        enemy_income: 35.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 26,
        ai_tick: 0.93,
        attack_delay: 3.7,
        enemy_power: 1.31,
    },
    // 83
    LevelSpec {
        player_base: (32, 41),
        enemies: &[EnemySpec { pos: (8, 8), style: Swarm }, EnemySpec { pos: (56, 9), style: Balanced }],
        water: &[(20, 24, 5.0), (44, 24, 5.0), (32, 14, 4.0)],
        ore: &[(10, 36, 4.0), (54, 36, 4.0)],
        rock_density: 0.955,
        player_credits: 1129.0,
        enemy_credits: 3490.0,
        enemy_income: 35.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 26,
        ai_tick: 0.93,
        attack_delay: 3.6,
        enemy_power: 1.32,
    },
    // 84
    LevelSpec {
        player_base: (55, 8),
        enemies: &[EnemySpec { pos: (8, 40), style: Swarm }],
        water: &[(30, 24, 6.0), (44, 14, 4.0), (18, 30, 4.0)],
        ore: &[(30, 40, 4.5)],
        rock_density: 0.955,
        player_credits: 1122.0,
        enemy_credits: 3520.0,
        enemy_income: 36.0,
        first_wave: 7,
        wave_growth: 3,
        wave_cap: 27,
        ai_tick: 0.92,
        attack_delay: 3.6,
        enemy_power: 1.32,
    },
    // 85
    LevelSpec {
        player_base: (8, 24),
        enemies: &[EnemySpec { pos: (54, 9), style: Armor }, EnemySpec { pos: (54, 40), style: Swarm }],
        water: &[(30, 24, 6.0), (40, 16, 4.0), (40, 34, 4.0), (22, 8, 4.0)],
        ore: &[(26, 40, 4.0), (28, 8, 4.0)],
        rock_density: 0.955,
        player_credits: 1115.0,
        enemy_credits: 3550.0,
        enemy_income: 36.0,
        first_wave: 8,
        wave_growth: 4,
        wave_cap: 28,
        ai_tick: 0.91,
        attack_delay: 3.5,
        enemy_power: 1.33,
    },
    // 86
    LevelSpec {
        player_base: (32, 8),
        enemies: &[EnemySpec { pos: (8, 40), style: Armor }, EnemySpec { pos: (56, 40), style: Swarm }],
        water: &[(20, 24, 5.0), (44, 24, 5.0), (32, 33, 4.0), (12, 12, 4.0)],
        ore: &[(32, 42, 4.0), (54, 14, 4.0)],
        rock_density: 0.955,
        player_credits: 1108.0,
        enemy_credits: 3580.0,
        enemy_income: 36.0,
        first_wave: 8,
        wave_growth: 4,
        wave_cap: 28,
        ai_tick: 0.9,
        attack_delay: 3.5,
        enemy_power: 1.34,
    },
    // 87
    LevelSpec {
        player_base: (55, 41),
        enemies: &[EnemySpec { pos: (8, 8), style: Swarm }, EnemySpec { pos: (10, 38), style: Balanced }],
        water: &[(34, 24, 6.0), (40, 40, 4.0), (26, 12, 4.0), (48, 18, 4.0)],
        ore: &[(40, 9, 4.0), (22, 40, 4.0)],
        rock_density: 0.955,
        player_credits: 1101.0,
        enemy_credits: 3610.0,
        enemy_income: 37.0,
        first_wave: 8,
        wave_growth: 4,
        wave_cap: 28,
        ai_tick: 0.89,
        attack_delay: 3.4,
        enemy_power: 1.34,
    },
    // 88
    LevelSpec {
        player_base: (50, 38),
        enemies: &[EnemySpec { pos: (9, 8), style: Armor }],
        water: &[(30, 24, 6.0), (20, 38, 4.0), (40, 12, 4.0)],
        ore: &[(28, 40, 4.5)],
        rock_density: 0.955,
        player_credits: 1094.0,
        enemy_credits: 3640.0,
        enemy_income: 37.0,
        first_wave: 8,
        wave_growth: 4,
        wave_cap: 29,
        ai_tick: 0.89,
        attack_delay: 3.4,
        enemy_power: 1.35,
    },
    // 89
    LevelSpec {
        player_base: (54, 8),
        enemies: &[EnemySpec { pos: (8, 40), style: Armor }, EnemySpec { pos: (28, 41), style: Swarm }],
        water: &[(34, 22, 6.0), (18, 24, 4.0), (46, 30, 4.0), (44, 10, 4.0)],
        ore: &[(10, 12, 4.0), (44, 42, 4.0)],
        rock_density: 0.955,
        player_credits: 1087.0,
        enemy_credits: 3670.0,
        enemy_income: 38.0,
        first_wave: 8,
        wave_growth: 4,
        wave_cap: 30,
        ai_tick: 0.88,
        attack_delay: 3.3,
        enemy_power: 1.35,
    },
    // 90
    LevelSpec {
        player_base: (56, 24),
        enemies: &[EnemySpec { pos: (8, 9), style: Armor }, EnemySpec { pos: (10, 40), style: Swarm }],
        water: &[(32, 24, 6.0), (24, 12, 4.0), (24, 36, 4.0), (42, 24, 4.0)],
        ore: &[(40, 40, 4.0), (40, 9, 4.0)],
        rock_density: 0.955,
        player_credits: 1080.0,
        enemy_credits: 3700.0,
        enemy_income: 38.0,
        first_wave: 8,
        wave_growth: 4,
        wave_cap: 30,
        ai_tick: 0.87,
        attack_delay: 3.3,
        enemy_power: 1.36,
    },
    // 91
    LevelSpec {
        player_base: (8, 40),
        enemies: &[EnemySpec { pos: (56, 8), style: Armor }, EnemySpec { pos: (36, 7), style: Swarm }],
        water: &[(30, 28, 5.0), (18, 18, 4.0), (48, 30, 5.0), (44, 18, 4.0)],
        ore: &[(22, 30, 4.0)],
        rock_density: 0.955,
        player_credits: 1072.0,
        enemy_credits: 3730.0,
        enemy_income: 38.0,
        first_wave: 8,
        wave_growth: 4,
        wave_cap: 30,
        ai_tick: 0.86,
        attack_delay: 3.3,
        enemy_power: 1.36,
    },
    // 92
    LevelSpec {
        player_base: (8, 8),
        enemies: &[EnemySpec { pos: (56, 40), style: Armor }, EnemySpec { pos: (18, 40), style: Swarm }],
        water: &[(32, 24, 5.0), (24, 16, 4.0), (40, 32, 5.0), (30, 36, 4.0)],
        ore: &[(40, 16, 4.5)],
        rock_density: 0.955,
        player_credits: 1064.0,
        enemy_credits: 3760.0,
        enemy_income: 39.0,
        first_wave: 8,
        wave_growth: 4,
        wave_cap: 31,
        ai_tick: 0.86,
        attack_delay: 3.2,
        enemy_power: 1.37,
    },
    // 93
    LevelSpec {
        player_base: (8, 24),
        enemies: &[EnemySpec { pos: (56, 24), style: Armor }],
        water: &[(32, 16, 5.0), (32, 32, 5.0), (44, 24, 4.0), (20, 12, 4.0)],
        ore: &[(32, 38, 4.0)],
        rock_density: 0.955,
        player_credits: 1056.0,
        enemy_credits: 3790.0,
        enemy_income: 39.0,
        first_wave: 8,
        wave_growth: 4,
        wave_cap: 31,
        ai_tick: 0.85,
        attack_delay: 3.2,
        enemy_power: 1.37,
    },
    // 94
    LevelSpec {
        player_base: (56, 40),
        enemies: &[EnemySpec { pos: (8, 8), style: Armor }, EnemySpec { pos: (8, 30), style: Swarm }],
        water: &[(32, 24, 5.0), (24, 32, 4.0), (40, 16, 4.0), (28, 12, 4.0)],
        ore: &[(40, 34, 4.0)],
        rock_density: 0.955,
        player_credits: 1048.0,
        enemy_credits: 3820.0,
        enemy_income: 40.0,
        first_wave: 8,
        wave_growth: 4,
        wave_cap: 32,
        ai_tick: 0.84,
        attack_delay: 3.2,
        enemy_power: 1.38,
    },
    // 95
    LevelSpec {
        player_base: (56, 8),
        enemies: &[EnemySpec { pos: (8, 40), style: Swarm }, EnemySpec { pos: (8, 16), style: Armor }],
        water: &[(32, 24, 5.0), (40, 36, 4.0), (24, 16, 4.0), (34, 12, 4.0)],
        ore: &[(30, 32, 4.0)],
        rock_density: 0.955,
        player_credits: 1040.0,
        enemy_credits: 3850.0,
        enemy_income: 40.0,
        first_wave: 8,
        wave_growth: 4,
        wave_cap: 32,
        ai_tick: 0.83,
        attack_delay: 3.1,
        enemy_power: 1.38,
    },
    // 96
    LevelSpec {
        player_base: (32, 41),
        enemies: &[EnemySpec { pos: (8, 8), style: Armor }, EnemySpec { pos: (56, 8), style: Swarm }],
        water: &[(20, 24, 5.0), (44, 24, 5.0), (32, 16, 4.0), (50, 36, 4.0)],
        ore: &[(32, 28, 4.0)],
        rock_density: 0.955,
        player_credits: 1032.0,
        enemy_credits: 3880.0,
        enemy_income: 40.0,
        first_wave: 8,
        wave_growth: 4,
        wave_cap: 32,
        ai_tick: 0.83,
        attack_delay: 3.1,
        enemy_power: 1.38,
    },
    // 97
    LevelSpec {
        player_base: (32, 8),
        enemies: &[EnemySpec { pos: (8, 40), style: Armor }, EnemySpec { pos: (56, 40), style: Swarm }],
        water: &[(20, 24, 5.0), (44, 24, 5.0), (32, 32, 4.0), (14, 14, 4.0)],
        ore: &[(32, 40, 4.5)],
        rock_density: 0.955,
        player_credits: 1024.0,
        enemy_credits: 3910.0,
        enemy_income: 41.0,
        first_wave: 8,
        wave_growth: 4,
        wave_cap: 33,
        ai_tick: 0.82,
        attack_delay: 3.1,
        enemy_power: 1.39,
    },
    // 98
    LevelSpec {
        player_base: (10, 38),
        enemies: &[EnemySpec { pos: (54, 10), style: Armor }],
        water: &[(32, 24, 5.0), (24, 30, 4.0), (40, 18, 4.0), (30, 12, 4.0)],
        ore: &[(34, 34, 4.0)],
        rock_density: 0.955,
        player_credits: 1016.0,
        enemy_credits: 3940.0,
        enemy_income: 41.0,
        first_wave: 8,
        wave_growth: 4,
        wave_cap: 33,
        ai_tick: 0.81,
        attack_delay: 3.1,
        enemy_power: 1.39,
    },
    // 99
    LevelSpec {
        player_base: (56, 24),
        enemies: &[EnemySpec { pos: (8, 12), style: Armor }, EnemySpec { pos: (8, 38), style: Swarm }],
        water: &[(32, 24, 5.0), (24, 14, 4.0), (24, 34, 4.0), (44, 34, 4.0)],
        ore: &[(40, 14, 4.0)],
        rock_density: 0.955,
        player_credits: 1008.0,
        enemy_credits: 3970.0,
        enemy_income: 42.0,
        first_wave: 8,
        wave_growth: 4,
        wave_cap: 34,
        ai_tick: 0.81,
        attack_delay: 3.0,
        enemy_power: 1.4,
    },
    // 100
    LevelSpec {
        player_base: (32, 41),
        enemies: &[EnemySpec { pos: (8, 9), style: Armor }, EnemySpec { pos: (56, 9), style: Swarm }],
        water: &[(20, 20, 5.0), (44, 20, 5.0), (16, 34, 4.0), (48, 34, 4.0)],
        ore: &[(12, 24, 4.0), (52, 24, 4.0)],
        rock_density: 0.955,
        player_credits: 1000.0,
        enemy_credits: 4000.0,
        enemy_income: 42.0,
        first_wave: 8,
        wave_growth: 4,
        wave_cap: 34,
        ai_tick: 0.8,
        attack_delay: 3.0,
        enemy_power: 1.4,
    },
];
// <<LEVELS_END>>

#[cfg(test)]
mod tests {
    use super::*;

    // En sammensatt "trussel-score" -- brukes for a sjekke at vanskeligheten
    // stiger jevnt (sma steg) over nivaene.
    fn threat_score(s: &LevelSpec) -> f32 {
        s.enemy_credits * 0.01
            + s.enemy_income * 0.6
            + s.wave_cap as f32 * 1.2
            + s.wave_growth as f32 * 1.5
            + s.first_wave as f32 * 0.4
            + 8.0 / s.ai_tick
            + 20.0 / s.attack_delay
            + s.enemy_power * 8.0
            + s.enemies.len() as f32 * 4.0
            - s.player_credits * 0.004
    }

    fn in_bounds(t: (i32, i32)) -> bool {
        t.0 >= 6 && t.1 >= 6 && (t.0 as usize) < MAP_W - 6 && (t.1 as usize) < MAP_H - 6
    }

    fn dist(a: (i32, i32), b: (i32, i32)) -> f32 {
        let (dx, dy) = ((a.0 - b.0) as f32, (a.1 - b.1) as f32);
        (dx * dx + dy * dy).sqrt()
    }

    // BFS over fremkommelig terreng (ikke vann/fjell) -- spilleren skal kunne na
    // hver fiendebase. Bygg-fotavtrykk ryddes til Grass av Game::new_level, sa vi
    // rydder ogsa her rundt basene for testen.
    fn reachable(map: &mut [Terrain], from: (i32, i32), to: (i32, i32)) -> bool {
        let clear = |m: &mut [Terrain], c: (i32, i32)| {
            for dy in -4..=4 {
                for dx in -4..=4 {
                    let (x, y) = (c.0 + dx, c.1 + dy);
                    if x >= 0 && y >= 0 && (x as usize) < MAP_W && (y as usize) < MAP_H {
                        m[y as usize * MAP_W + x as usize] = Terrain::Grass;
                    }
                }
            }
        };
        clear(map, from);
        clear(map, to);
        let idx = |x: usize, y: usize| y * MAP_W + x;
        let passable = |t: Terrain| t != Terrain::Water && t != Terrain::Rock;
        let mut seen = vec![false; MAP_W * MAP_H];
        let mut stack = vec![(from.0 as usize, from.1 as usize)];
        seen[idx(from.0 as usize, from.1 as usize)] = true;
        while let Some((x, y)) = stack.pop() {
            if (x, y) == (to.0 as usize, to.1 as usize) {
                return true;
            }
            let nb = [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ];
            for (nx, ny) in nb {
                if nx < MAP_W && ny < MAP_H && !seen[idx(nx, ny)] && passable(map[idx(nx, ny)]) {
                    seen[idx(nx, ny)] = true;
                    stack.push((nx, ny));
                }
            }
        }
        false
    }

    #[test]
    fn alle_baner_er_gyldige() {
        for (i, s) in LEVELS.iter().enumerate() {
            let lvl = i + 1;
            assert!(in_bounds(s.player_base), "niva {} spillerbase utenfor", lvl);
            assert!(!s.enemies.is_empty(), "niva {} mangler fiende", lvl);
            assert!(!s.ore.is_empty(), "niva {} mangler malm", lvl);
            assert!(s.player_credits > 0.0 && s.enemy_credits > 0.0, "niva {} kreditter", lvl);
            assert!(s.wave_cap >= s.first_wave, "niva {} wave_cap < first_wave", lvl);
            assert!(s.ai_tick > 0.0 && s.attack_delay > 0.0, "niva {} timing", lvl);
            for e in s.enemies {
                assert!(in_bounds(e.pos), "niva {} fiendebase utenfor", lvl);
                assert!(dist(s.player_base, e.pos) >= 24.0, "niva {} baser for naer", lvl);
            }
            // ingen to fiendebaser oppa hverandre
            for a in 0..s.enemies.len() {
                for b in (a + 1)..s.enemies.len() {
                    assert!(dist(s.enemies[a].pos, s.enemies[b].pos) >= 12.0, "niva {} fiender overlapper", lvl);
                }
            }
            // stiforbarhet: spiller -> hver fiendebase
            for e in s.enemies {
                let mut map = gen_map_for(s);
                assert!(reachable(&mut map, s.player_base, e.pos), "niva {} ikke stiforbar til {:?}", lvl, e.pos);
            }
        }
    }

    #[test]
    fn vanskelighet_stiger_jevnt() {
        let mut prev = threat_score(&LEVELS[0]);
        for (i, s) in LEVELS.iter().enumerate().skip(1) {
            let sc = threat_score(s);
            // Stort sett stigende i sma steg. Antall fiender varierer for
            // variasjon (+/-4), sa vi tillater et lite dipp men ikke store hopp.
            assert!(sc >= prev - 6.0, "niva {} mye lettere enn forrige ({:.1} < {:.1})", i + 1, sc, prev);
            assert!(sc <= prev + 14.0, "niva {} for stort hopp ({:.1} -> {:.1})", i + 1, prev, sc);
            prev = sc;
        }
    }
}

#!/usr/bin/env python3
"""Sett sammen 100 nivaer: kart-oppsett fra agent-JSON + jevn vanskelighetskurve.

Leser scratchpad/levels_band_*.json (hver = liste med niva-objekter):
  {level, player_base:[x,y], enemies:[{pos:[x,y],style:"Balanced|Armor|Swarm"}],
   water:[[x,y,r],...], ore:[[x,y,r],...], rock_density:float}
Skriver LEVELS-blokken (mellom sentinels) i src/levels.rs.
"""
import glob, json, os, re

SP = os.path.dirname(os.path.abspath(__file__))
ROOT = "/Users/punnerud/Downloads/openrarust"
LEVELS_RS = os.path.join(ROOT, "src", "levels.rs")

# Vanskelighets-ankre: level -> (pc, ec, inc, fw, g, cap, tick, delay, pow)
ANCH = {
  1:   (1600, 900,  7,  5,1,6,  2.2, 14.0, 0.85),
  10:  (1550, 1200, 11, 5,1,7,  1.9, 11.0, 0.92),
  20:  (1500, 1500, 14, 5,2,9,  1.7, 9.0,  1.00),
  30:  (1450, 1850, 18, 5,2,11, 1.5, 7.5,  1.06),
  40:  (1400, 2100, 21, 6,2,13, 1.35,6.5,  1.10),
  50:  (1350, 2400, 24, 6,3,15, 1.25,5.5,  1.15),
  60:  (1300, 2700, 27, 6,3,18, 1.15,5.0,  1.20),
  70:  (1250, 3000, 30, 7,3,21, 1.05,4.3,  1.25),
  80:  (1150, 3400, 34, 7,3,25, 0.95,3.8,  1.30),
  90:  (1080, 3700, 38, 8,4,30, 0.87,3.3,  1.36),
  100: (1000, 4000, 42, 8,4,34, 0.80,3.0,  1.40),
}
KEYS = list(sorted(ANCH))

def lerp(a, b, t): return a + (b - a) * t

def difficulty(level):
    # finn omkringliggende ankre
    lo = max(k for k in KEYS if k <= level)
    hi = min(k for k in KEYS if k >= level)
    if lo == hi:
        v = ANCH[lo]
    else:
        t = (level - lo) / (hi - lo)
        v = tuple(lerp(ANCH[lo][i], ANCH[hi][i], t) for i in range(9))
    pc, ec, inc, fw, g, cap, tick, delay, pow_ = v
    return dict(
        player_credits=round(pc), enemy_credits=round(ec), enemy_income=round(inc),
        first_wave=round(fw), wave_growth=round(g), wave_cap=round(cap),
        ai_tick=round(tick, 2), attack_delay=round(delay, 1), enemy_power=round(pow_, 2),
    )

def clampi(v, lo, hi): return max(lo, min(hi, int(round(v))))

def load_bands():
    out = {}
    files = sorted(glob.glob(os.path.join(SP, "levels_band_*.json")))
    for f in files:
        data = json.load(open(f, encoding="utf-8"))
        for lv in data:
            out[int(lv["level"])] = lv
    return out

def fmt_blobs(blobs):
    parts = []
    for b in blobs or []:
        x = clampi(b[0], 1, 62); y = clampi(b[1], 1, 46); r = round(float(b[2]), 1)
        r = max(2.5, min(6.0, r))
        parts.append(f"({x}, {y}, {r})")
    return "&[" + ", ".join(parts) + "]"

def main():
    bands = load_bands()
    missing = [n for n in range(1, 101) if n not in bands]
    if missing:
        raise SystemExit(f"Mangler nivaer: {missing}")
    entries = []
    for n in range(1, 101):
        lv = bands[n]
        d = difficulty(n)
        px = clampi(lv["player_base"][0], 6, 57); py = clampi(lv["player_base"][1], 6, 41)
        ens = []
        for e in lv["enemies"]:
            ex = clampi(e["pos"][0], 6, 57); ey = clampi(e["pos"][1], 6, 41)
            st = e.get("style", "Balanced")
            if st not in ("Balanced", "Armor", "Swarm"): st = "Balanced"
            ens.append(f"EnemySpec {{ pos: ({ex}, {ey}), style: {st} }}")
        enemies = "&[" + ", ".join(ens) + "]"
        water = fmt_blobs(lv.get("water"))
        ore = fmt_blobs(lv.get("ore"))
        if ore == "&[]":  # sikkerhet: alltid minst en malm
            ore = "&[(32, 24, 4.0)]"
        rock = round(float(lv.get("rock_density", 0.97)), 3)
        rock = max(0.95, min(0.99, rock))
        e = f"""    // {n}
    LevelSpec {{
        player_base: ({px}, {py}),
        enemies: {enemies},
        water: {water},
        ore: {ore},
        rock_density: {rock},
        player_credits: {d['player_credits']}.0,
        enemy_credits: {d['enemy_credits']}.0,
        enemy_income: {d['enemy_income']}.0,
        first_wave: {d['first_wave']},
        wave_growth: {d['wave_growth']},
        wave_cap: {d['wave_cap']},
        ai_tick: {d['ai_tick']},
        attack_delay: {d['attack_delay']},
        enemy_power: {d['enemy_power']},
    }},"""
        entries.append(e)
    block = "// <<LEVELS_START>>\npub const LEVELS: &[LevelSpec] = &[\n" + "\n".join(entries) + "\n];\n// <<LEVELS_END>>"
    src = open(LEVELS_RS, encoding="utf-8").read()
    src = re.sub(r"// <<LEVELS_START>>.*?// <<LEVELS_END>>", block, src, flags=re.S)
    open(LEVELS_RS, "w", encoding="utf-8").write(src)
    print(f"Skrev 100 nivaer til {LEVELS_RS}")

if __name__ == "__main__":
    main()

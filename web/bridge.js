// Tynt web-skall for OpenRA Rust.
//
// ALL UI (joystick, zoom, burger, dev-meny, sprakvelger, HUD) tegnes og
// handteres na i Rust/macroquad slik at spillet er native-klart (iPhone/
// Android/desktop) uten JS/HTML-overlay. Dette skallet gjor kun det
// nettleseren ikke kan gjore fra WASM enna:
//   1) Web Audio: prosedyrale lydeffekter (ingen lydfiler).
//   2) Hindre nettleserens kontekstmeny pa hoyreklikk.
//   3) Fortelle spillet nar pekeren forlater vinduet (stopp kant-scroll).
//
// Lastes ETTER mq_js_bundle.js, men FOR load("openrarust.wasm").

(function () {
    "use strict";

    // ---- Hoyreklikk brukes i spillet -> ikke vis "Lagre bilde"-menyen. ----
    window.addEventListener("contextmenu", (e) => e.preventDefault());

    // ---- Web Audio: prosedyrale lyder (ingen lydfiler/lisens) -----------
    // AudioContext kan kun startes etter en bruker-gest -> lat init.
    let actx = null;
    const lastPlay = {}; // enkel rate-limit pr. lyd-id
    function audio() {
        if (!actx) {
            try { actx = new (window.AudioContext || window.webkitAudioContext)(); } catch (e) { return null; }
        }
        if (actx.state === "suspended") actx.resume();
        return actx;
    }
    // Engangs-gest for a vekke lyd pa iOS/Safari.
    const wake = () => { audio(); window.removeEventListener("pointerdown", wake); window.removeEventListener("keydown", wake); };
    window.addEventListener("pointerdown", wake);
    window.addEventListener("keydown", wake);

    function tone(t0, freq, dur, type, gain, freqEnd) {
        const ac = actx;
        const o = ac.createOscillator();
        const g = ac.createGain();
        o.type = type || "sine";
        o.frequency.setValueAtTime(freq, t0);
        if (freqEnd) o.frequency.exponentialRampToValueAtTime(Math.max(1, freqEnd), t0 + dur);
        g.gain.setValueAtTime(0.0001, t0);
        g.gain.exponentialRampToValueAtTime(gain, t0 + 0.008);
        g.gain.exponentialRampToValueAtTime(0.0001, t0 + dur);
        o.connect(g).connect(ac.destination);
        o.start(t0); o.stop(t0 + dur + 0.02);
    }
    function noise(t0, dur, gain, lp) {
        const ac = actx;
        const n = Math.floor(ac.sampleRate * dur);
        const buf = ac.createBuffer(1, n, ac.sampleRate);
        const d = buf.getChannelData(0);
        for (let i = 0; i < n; i++) d[i] = (Math.random() * 2 - 1) * (1 - i / n);
        const src = ac.createBufferSource(); src.buffer = buf;
        const g = ac.createGain(); g.gain.setValueAtTime(gain, t0); g.gain.exponentialRampToValueAtTime(0.0001, t0 + dur);
        const f = ac.createBiquadFilter(); f.type = "lowpass"; f.frequency.value = lp || 2000;
        src.connect(f).connect(g).connect(ac.destination);
        src.start(t0); src.stop(t0 + dur);
    }
    // id: 1=skudd 2=eksplosjon 3=plasser 4=ferdig 5=lossing 6=tarn 7=seier 8=tap
    const MINGAP = { 1: 60, 2: 90, 6: 80 }; // ms mellom like lyder
    function playSound(id) {
        const ac = audio(); if (!ac) return;
        const now = (typeof performance !== "undefined" ? performance.now() : 0);
        const gap = MINGAP[id] || 0;
        if (gap && lastPlay[id] && now - lastPlay[id] < gap) return;
        lastPlay[id] = now;
        const t = ac.currentTime;
        switch (id) {
            case 1: tone(t, 320, 0.08, "square", 0.06, 120); break;               // skudd
            case 2: noise(t, 0.45, 0.5, 900); tone(t, 90, 0.4, "sawtooth", 0.18, 35); break; // eksplosjon
            case 3: tone(t, 220, 0.12, "triangle", 0.16, 440); break;             // plassert bygg
            case 4: tone(t, 520, 0.1, "sine", 0.16); tone(t + 0.1, 780, 0.12, "sine", 0.16); break; // enhet klar
            case 5: tone(t, 160, 0.18, "sine", 0.14, 90); break;                  // lossing
            case 6: tone(t, 200, 0.18, "sawtooth", 0.16, 70); noise(t, 0.12, 0.18, 1200); break; // tarn-storkule
            case 7: [523, 659, 784, 1047].forEach((f, i) => tone(t + i * 0.12, f, 0.18, "triangle", 0.2)); break; // seier
            case 8: [392, 330, 262, 196].forEach((f, i) => tone(t + i * 0.14, f, 0.22, "sawtooth", 0.18)); break; // tap
        }
    }

    // ---- Kant-scroll: stopp nar pekeren forlater vinduet -----------------
    // Spillet leser dette via kommando-koen (kode 12). Vi legger en bitteliten
    // ko som WASM tommer via js_poll_cmd/js_poll_arg.
    const queue = [];
    const leave = () => queue.push({ code: 12, args: [0, 0, 0, 0] });
    window.addEventListener("mouseout", (e) => { if (!e.relatedTarget) leave(); });
    window.addEventListener("blur", leave);
    document.addEventListener("mouseleave", leave);

    // ---- WASM <-> JS broen (numeriske externs) --------------------------
    function register(importObject) {
        const env = importObject.env;
        // Telemetri/overlay er flyttet inn i Rust -> rapport-funksjonene er no-op.
        env.js_report = function () {};
        env.js_report_econ = function () {};
        env.js_report_fog = function () {};
        // Kommando-ko: kun "pekeren forlot vinduet" sendes fra JS na.
        env.js_poll_arg = function (i) { return queue[0] ? queue[0].args[i] : 0; };
        env.js_poll_cmd = function () { const f = queue.shift(); return f ? f.code : 0; };
        // Spillet ber om en lyd (prosedyral Web Audio-synth, ingen lydfiler).
        env.js_sound = function (id) { try { playSound(id | 0); } catch (e) {} };
        // Vedvarende lagring: fremgang + innstillinger i localStorage. Numerisk
        // (en i32 pr. nokkel). Fraverende nokkel -> i32::MIN (-2147483648) som Rust
        // tolker som "ingen lagret verdi". Feiler stille (privat modus e.l.).
        env.js_store_get = function (key) {
            try {
                const v = localStorage.getItem("mpera_" + (key | 0));
                return v === null ? -2147483648 : (parseInt(v, 10) | 0);
            } catch (e) { return -2147483648; }
        };
        env.js_store_set = function (key, val) {
            try { localStorage.setItem("mpera_" + (key | 0), String(val | 0)); } catch (e) {}
        };
    }

    if (typeof miniquad_add_plugin === "function") {
        miniquad_add_plugin({ register_plugin: register, version: 1, name: "openra_bridge" });
    } else {
        console.error("[bridge] miniquad_add_plugin mangler — last bridge.js ETTER mq_js_bundle.js");
    }
})();

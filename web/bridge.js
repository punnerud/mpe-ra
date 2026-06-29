// JS <-> WASM-bro + dev-panel for OpenRA Rust.
//
// - Tar imot tilstand fra spillet (js_report / js_report_econ) -> overlay + telemetri.
// - Sender kommandoer til spillet (js_poll_cmd / js_poll_arg) via window.openra.
//
// Lastes ETTER mq_js_bundle.js, men FOR load("openrarust.wasm").

(function () {
    "use strict";

    // Enhetstyper: 0 = infanteri, 1 = stridsvogn, 2 = hoster.
    const openra = {
        state: {},
        telemetry: true,
        _queue: [], // ko av kommandoer -> spillet tommer en eller flere pr. frame
        _send(code, a0 = 0, a1 = 0, a2 = 0, a3 = 0) {
            this._queue.push({ code, args: [a0, a1, a2, a3] });
        },
        restart() { this._send(1); },
        center() { this._send(2); },
        setCam(x, y) { this._send(3, x, y); },
        setZoom(z) { this._send(4, z); },
        spawn(team, kind, x, y) { this._send(5, team, kind, x, y); },
        build(kind) { this._send(6, kind); },
        give(team, amount) { this._send(7, team, amount); },
        flag(id, on) { this._send(8, id, on ? 1 : 0); },
        pause(on) { this.flag(0, on); },
        freeBuild(on) { this.flag(1, on); },
        god(on) { this.flag(2, on); },
        speed(s) { this._send(9, s); },
        setRally(x, y) { this._send(10, x, y); },
        moveAll(x, y) { this._send(11, x, y); },
        reveal(on) { this.flag(3, on); },
        pointerLeft() { this._send(12); }, // stopp kant-scroll nar pekeren er utenfor
        placeMode(kind) { this._send(13, kind); }, // 0=RAF 1=FAB 2=Gjerde 3=Tarn
        placeBuilding(kind, x, y) { this._send(14, kind, x, y); }, // plasser direkte
        setLang(i) { this._send(15, i); }, // velg sprak (indeks i LANGS)
    };
    window.openra = openra;

    // Gjeldende sprak + oversettelseshjelper (bruker web/i18n.js fra ordboken).
    let curLang = 0;
    const TR = (k) => (window.I18Nt ? window.I18Nt(curLang, k) : k);

    // Sprakliste [flagg, lokalt navn, engelsk navn] -- MA ha samme rekkefolge
    // som LANGS i src/i18n.rs (indeks = setLang-kommando).
    const LANGS = [
        ["🇬🇧", "English", "English"], ["🇳🇴", "Norsk", "Norwegian"], ["🇸🇪", "Svenska", "Swedish"], ["🇩🇰", "Dansk", "Danish"],
        ["🇩🇪", "Deutsch", "German"], ["🇳🇱", "Nederlands", "Dutch"], ["🇫🇷", "Français", "French"], ["🇪🇸", "Español", "Spanish"],
        ["🇲🇽", "Español (MX)", "Spanish (Mexico)"], ["🇵🇹", "Português", "Portuguese"], ["🇧🇷", "Português (BR)", "Portuguese (Brazil)"], ["🇮🇹", "Italiano", "Italian"],
        ["🇬🇷", "Ελληνικά", "Greek"], ["🇫🇮", "Suomi", "Finnish"], ["🇵🇱", "Polski", "Polish"], ["🇨🇿", "Čeština", "Czech"],
        ["🇸🇰", "Slovenčina", "Slovak"], ["🇭🇺", "Magyar", "Hungarian"], ["🇷🇴", "Română", "Romanian"], ["🇭🇷", "Hrvatski", "Croatian"],
        ["🇸🇮", "Slovenščina", "Slovenian"], ["🇧🇬", "Български", "Bulgarian"], ["🇺🇦", "Українська", "Ukrainian"], ["🇹🇷", "Türkçe", "Turkish"],
        ["🇮🇸", "Íslenska", "Icelandic"], ["🇪🇪", "Eesti", "Estonian"], ["🇱🇻", "Latviešu", "Latvian"], ["🇱🇹", "Lietuvių", "Lithuanian"],
        ["🇮🇪", "Gaeilge", "Irish"], ["🏴", "Català", "Catalan"], ["🇨🇳", "简体中文", "Chinese (Simplified)"], ["🇹🇼", "繁體中文", "Chinese (Traditional)"],
        ["🇯🇵", "日本語", "Japanese"], ["🇰🇷", "한국어", "Korean"], ["🇮🇳", "हिन्दी", "Hindi"], ["🇹🇭", "ไทย", "Thai"],
        ["🇻🇳", "Tiếng Việt", "Vietnamese"], ["🇮🇩", "Indonesia", "Indonesian"], ["🇲🇾", "Melayu", "Malay"], ["🇵🇭", "Filipino", "Filipino"],
        ["🇧🇩", "বাংলা", "Bengali"], ["🇸🇦", "العربية", "Arabic"], ["🇮🇱", "עברית", "Hebrew"], ["🇮🇷", "فارسی", "Persian"],
    ];

    // Kant-scroll skal kun virke nar pekeren er pa skjermen. Nar den forlater
    // vinduet slutter nettleseren a sende musebevegelser, og spillet ville
    // ellers fortsette a scrolle mot siste kant. Si fra til spillet.
    const leave = () => openra.pointerLeft();
    window.addEventListener("mouseout", (e) => { if (!e.relatedTarget) leave(); });
    window.addEventListener("blur", leave);
    document.addEventListener("mouseleave", leave);

    // Hoyreklikk i spillet brukes til a fjerne markering -> ikke vis nettleserens
    // kontekstmeny ("Lagre bilde" osv.).
    window.addEventListener("contextmenu", (e) => e.preventDefault());

    // ---- Web Audio: prosedyrale lyder (ingen lydfiler/lisens) -----------
    // AudioContext kan kun startes etter en bruker-gest -> lat init.
    let actx = null, muted = false;
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
        if (muted) return;
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

    // ---- Debug-overlay (oppe til hoyre) --------------------------------
    const css = (el, s) => { el.style.cssText = s; return el; };
    // Plassert oppe til venstre (under HUD-linjen) slik at det ikke dekker
    // canvas-sidebaren med minikart + byggmeny til hoyre. Skjult som standard
    // -- vises kun nar dev-panelet apnes.
    const panel = css(document.createElement("div"),
        "position:fixed;top:40px;left:10px;z-index:1000;font:12px/1.5 ui-monospace,Menlo,monospace;" +
        "background:rgba(0,0,0,.66);color:#cfe;padding:8px 11px;border:1px solid #2a4;border-radius:8px;" +
        "min-width:200px;pointer-events:none;white-space:pre;display:none");
    panel.textContent = "...";
    let panelLive = false; // settes nar live spilldata kommer (js_report)

    // ---- Dev-panel (nede til venstre, klikkbart) -----------------------
    const dev = css(document.createElement("div"),
        "position:fixed;bottom:46px;left:10px;z-index:1001;font:13px system-ui,sans-serif;" +
        "background:rgba(10,20,10,.92);color:#cfe;padding:10px;border:1px solid #2a4;border-radius:8px;" +
        "min-width:230px;display:none");
    dev.innerHTML =
        "<div id='dev-title' style='font-weight:700;margin-bottom:6px'>Dev panel</div>" +
        "<div id='dev-econ' style='margin-bottom:4px;font:12px ui-monospace,monospace'></div>" +
        "<div id='dev-fog' style='margin-bottom:8px;font:12px ui-monospace,monospace;color:#9cb'></div>" +
        "<div id='dev-btns' style='display:flex;flex-wrap:wrap;gap:5px'></div>";

    const toggle = css(document.createElement("button"),
        "position:fixed;bottom:10px;left:10px;z-index:1002;font:12px system-ui,sans-serif;" +
        "background:#1c2f1c;color:#cfe;border:1px solid #2a4;border-radius:6px;padding:5px 9px;cursor:pointer");
    toggle.textContent = "Dev ▸";

    // "Cheater"-merke nede til hoyre nar dev/juks er tatt i bruk.
    const cheatBadge = css(document.createElement("div"),
        "position:fixed;top:10px;right:10px;z-index:1002;font:bold 13px system-ui,sans-serif;" +
        "background:rgba(120,20,20,.85);color:#ffd;border:1px solid #e55;border-radius:6px;" +
        "padding:4px 9px;display:none;pointer-events:none;letter-spacing:.5px");
    cheatBadge.textContent = "Cheater";
    function markCheater() {
        cheatBadge.style.display = "block";
        try { localStorage.setItem("openra_cheat_ack", "1"); } catch (e) {}
    }

    toggle.onclick = () => {
        const open = dev.style.display === "none";
        // Forste gang dev tas i bruk: advar om at det er juks.
        if (open) {
            let acked = false;
            try { acked = localStorage.getItem("openra_cheat_ack") === "1"; } catch (e) {}
            if (!acked) {
                const ok = window.confirm(
                    "Dev/cheat mode\n\n" +
                    "These tools (free build, invincibility, instant units, reveal map) " +
                    "are cheats. Continue?\n\n" +
                    "Dev-/juksemodus: gratis bygg, udodelighet, gratis enheter, avslort kart. Fortsette?");
                if (!ok) return; // avbryt -> ikke apne
                markCheater();
            }
        }
        dev.style.display = open ? "block" : "none";
        toggle.style.display = open ? "none" : "block";
        // Live FPS/debug-overlayen + hurtigknappene (#controls) vises kun nar
        // dev-panelet er apent.
        panel.style.display = open ? "block" : "none";
        const ctrls = document.getElementById("controls");
        if (ctrls) ctrls.style.display = open ? "flex" : "none";
    };

    function mkBtn(label, fn) {
        const b = css(document.createElement("button"),
            "background:#1c2f1c;color:#cfe;border:1px solid #2a4;border-radius:6px;padding:4px 8px;cursor:pointer");
        b.textContent = label;
        b.onclick = fn;
        return b;
    }
    let paused = false, free = false, god = false, reveal = false;
    // Etikett for en av/på-knapp: "Label: on/off".
    const onoff = (key, state) => TR(key) + ": " + (state ? TR("DevOn") : TR("DevOff"));
    function buildDevButtons() {
        const wrap = dev.querySelector("#dev-btns");
        wrap.textContent = ""; // bygg om (brukes ogsa ved spraakbytte)
        const add = (l, f) => wrap.appendChild(mkBtn(l, f));
        add(TR("DevClose") + " ◂", () => { dev.style.display = "none"; toggle.style.display = "block"; });
        add(TR("DevRestart"), () => openra.restart());
        add(TR("DevCenter"), () => openra.center());
        add(TR("DevGive"), () => openra.give(0, 5000));
        add(TR("DevBuildInf"), () => openra.build(0));
        add(TR("DevBuildTank"), () => openra.build(1));
        add(TR("DevBuildHarv"), () => openra.build(2));
        add(TR("DevSpawnTankYou"), () => openra.spawn(0, 1, openra.state.cam.x + 400, openra.state.cam.y + 300));
        add(TR("DevSpawnTankFoe"), () => openra.spawn(1, 1, openra.state.cam.x + 400, openra.state.cam.y + 300));
        add(onoff("DevPause", paused), function () { paused = !paused; openra.pause(paused); this.textContent = onoff("DevPause", paused); });
        add(onoff("DevFreeBuild", free), function () { free = !free; openra.freeBuild(free); this.textContent = onoff("DevFreeBuild", free); });
        add(onoff("DevGod", god), function () { god = !god; openra.god(god); this.textContent = onoff("DevGod", god); });
        add(TR("DevSpeed") + " x1", () => openra.speed(1));
        add(TR("DevSpeed") + " x2", () => openra.speed(2));
        add(TR("DevSpeed") + " x4", () => openra.speed(4));
        add(onoff("DevReveal", reveal), function () { reveal = !reveal; openra.reveal(reveal); this.textContent = onoff("DevReveal", reveal); });
        add(onoff("DevSound", !muted), function () { muted = !muted; this.textContent = onoff("DevSound", !muted); });
    }

    // Oppdater all dev-/HTML-tekst til gjeldende sprak.
    function relocalize() {
        const title = dev.querySelector("#dev-title");
        if (title) title.textContent = TR("DevTitle");
        buildDevButtons();
        flagBtn.title = TR("ChooseLang");
        search.placeholder = TR("SearchLang");
        if (!panelLive) panel.textContent = TR("DevWaiting");
        // #controls-baren i index.html (med hurtigtast-suffiks).
        const setTxt = (id, s) => { const e = document.getElementById(id); if (e) e.textContent = s; };
        setTxt("ctl-restart", TR("DevRestart") + " (R)");
        setTxt("ctl-center", TR("DevCenter"));
        setTxt("ctl-zoom", TR("DevZoom") + " 1.0");
        setTxt("ctl-inf", TR("DevBuildInf") + " (1)");
        setTxt("ctl-tank", TR("DevBuildTank") + " (2)");
        setTxt("ctl-harv", TR("DevBuildHarv") + " (3)");
        const ld = document.getElementById("loading");
        if (ld) ld.textContent = TR("Loading");
    }

    // ---- Flaggvelger: kollapset = kun flagg; apnet = sokbar liste -----
    // Knapp som viser KUN gjeldende flagg. Plassert NEDERST til venstre (ved
    // siden av Dev-knappen) sa den ikke dekker kreditter/poeng oppe.
    const flagBtn = css(document.createElement("button"),
        "position:fixed;bottom:10px;left:66px;z-index:1003;font:22px system-ui,sans-serif;line-height:1;" +
        "background:rgba(10,20,10,.92);color:#cfe;border:1px solid #2a4;border-radius:8px;padding:5px 8px;cursor:pointer");
    flagBtn.title = "Velg sprak / choose language";

    // Liste som apnes OPPOVER fra knappen (forankret nederst -> vokser opp).
    const langPanel = css(document.createElement("div"),
        "position:fixed;bottom:52px;left:10px;z-index:1004;width:240px;display:none;" +
        "background:rgba(8,16,8,.97);border:1px solid #2a4;border-radius:8px;padding:8px;" +
        "box-shadow:0 -6px 20px rgba(0,0,0,.5)");
    const search = css(document.createElement("input"),
        "width:100%;box-sizing:border-box;font:13px system-ui,sans-serif;margin-top:6px;" +
        "background:#0c160c;color:#cfe;border:1px solid #2a4;border-radius:6px;padding:6px 8px;outline:none");
    search.placeholder = "Søk språk / search…";
    const list = css(document.createElement("div"),
        "max-height:320px;overflow-y:auto;display:flex;flex-direction:column;gap:2px");
    // Liste over, sokefelt under (nærmest knappen) siden panelet apnes oppover.
    langPanel.append(list, search);

    function renderList(filter) {
        list.textContent = "";
        const f = (filter || "").trim().toLowerCase();
        LANGS.forEach(([flag, native, english], i) => {
            if (f && !(english.toLowerCase().includes(f) || native.toLowerCase().includes(f))) return;
            const row = css(document.createElement("button"),
                "display:flex;align-items:center;gap:8px;width:100%;text-align:left;cursor:pointer;" +
                "font:13px system-ui,sans-serif;border:0;border-radius:6px;padding:6px 8px;" +
                (i === curLang ? "background:#1c3a1c;color:#dff;" : "background:transparent;color:#cfe;"));
            row.innerHTML = "<span style='font-size:18px'>" + flag + "</span>" +
                "<span style='flex:1'>" + english + "</span>" +
                "<span style='opacity:.6'>" + native + "</span>";
            row.onmouseenter = () => { if (i !== curLang) row.style.background = "#132613"; };
            row.onmouseleave = () => { if (i !== curLang) row.style.background = "transparent"; };
            row.onclick = () => { applyLang(i, true); closePanel(); };
            list.appendChild(row);
        });
    }
    function openPanel() {
        langPanel.style.display = "block";
        search.value = "";
        renderList("");
        search.focus();
    }
    function closePanel() { langPanel.style.display = "none"; }
    flagBtn.onclick = (e) => {
        e.stopPropagation();
        if (langPanel.style.display === "none") openPanel(); else closePanel();
    };
    search.oninput = () => renderList(search.value);
    document.addEventListener("click", (e) => {
        if (langPanel.style.display !== "none" && !langPanel.contains(e.target) && e.target !== flagBtn) closePanel();
    });

    function applyLang(i, save) {
        i = Math.max(0, Math.min(LANGS.length - 1, i | 0));
        curLang = i;
        flagBtn.textContent = LANGS[i][0]; // kun flagget
        openra.setLang(i);
        relocalize(); // oppdater all dev-/HTML-tekst til nytt sprak
        if (save) { try { localStorage.setItem("openra_lang", String(i)); } catch (e) {} }
    }

    // ---- Mobilkontroller: kamera-joystick + zoom-knapper ----------------
    // Joystick (nede til hoyre, Minecraft/Roblox-stil) panorerer kameraet.
    // right:162px -> til venstre for sidebaren (150px) sa den ikke dekker
    // byggknappene.
    const joyBase = css(document.createElement("div"),
        "position:fixed;bottom:24px;right:162px;z-index:1003;width:120px;height:120px;border-radius:50%;" +
        "background:rgba(20,30,20,.35);border:2px solid rgba(120,200,120,.5);touch-action:none;" +
        "user-select:none;-webkit-user-select:none");
    const joyKnob = css(document.createElement("div"),
        "position:absolute;left:50%;top:50%;width:52px;height:52px;border-radius:50%;" +
        "transform:translate(-50%,-50%);background:rgba(120,200,120,.55);border:2px solid rgba(200,255,200,.7);" +
        "pointer-events:none");
    joyBase.appendChild(joyKnob);

    let joyVec = { x: 0, y: 0 }, joyId = null, joyRAF = null;
    const R = 46; // maks knott-utslag (px)
    function joyUpdate(e) {
        const r = joyBase.getBoundingClientRect();
        let dx = e.clientX - (r.left + r.width / 2);
        let dy = e.clientY - (r.top + r.height / 2);
        const len = Math.hypot(dx, dy) || 1;
        const cl = Math.min(len, R);
        dx = dx / len * cl; dy = dy / len * cl;
        joyKnob.style.transform = "translate(calc(-50% + " + dx + "px), calc(-50% + " + dy + "px))";
        joyVec = { x: dx / R, y: dy / R };
    }
    function joyLoop() {
        if (joyId === null) return;
        const s = openra.state;
        if (s && s.cam) {
            const zoom = s.zoom || 1;
            const PAN = 16 / zoom; // verdens-piksler pr. frame ved fullt utslag
            openra.setCam(Math.round(s.cam.x + joyVec.x * PAN * zoom), Math.round(s.cam.y + joyVec.y * PAN * zoom));
        }
        joyRAF = requestAnimationFrame(joyLoop);
    }
    joyBase.addEventListener("pointerdown", (e) => {
        joyId = e.pointerId; joyBase.setPointerCapture(joyId);
        joyUpdate(e); if (joyRAF === null) joyLoop(); e.preventDefault();
    });
    joyBase.addEventListener("pointermove", (e) => { if (e.pointerId === joyId) joyUpdate(e); });
    const joyEnd = (e) => {
        if (e.pointerId !== joyId) return;
        joyId = null; if (joyRAF) cancelAnimationFrame(joyRAF); joyRAF = null;
        joyVec = { x: 0, y: 0 }; joyKnob.style.transform = "translate(-50%,-50%)";
    };
    joyBase.addEventListener("pointerup", joyEnd);
    joyBase.addEventListener("pointercancel", joyEnd);

    // Zoom +/- (oppe til venstre, under HUD-linjen).
    const zoomWrap = css(document.createElement("div"),
        "position:fixed;top:38px;left:10px;z-index:1003;display:flex;flex-direction:column;gap:4px");
    const zoomBtn = (txt, fn) => {
        const b = css(document.createElement("button"),
            "width:34px;height:34px;font:bold 20px system-ui,sans-serif;cursor:pointer;" +
            "background:rgba(20,30,20,.8);color:#cfe;border:1px solid #2a4;border-radius:7px;touch-action:manipulation");
        b.textContent = txt; b.onclick = fn; return b;
    };
    const curZoom = () => (openra.state && openra.state.zoom) ? openra.state.zoom : 1;
    zoomWrap.append(
        zoomBtn("+", () => openra.setZoom(Math.min(3.0, curZoom() * 1.25))),
        zoomBtn("−", () => openra.setZoom(Math.max(0.4, curZoom() / 1.25))),
    );

    const mount = () => {
        if (!document.body) return;
        document.body.append(panel, dev, toggle, flagBtn, langPanel, cheatBadge, joyBase, zoomWrap);
        buildDevButtons();
        // Vis "Cheater" igjen hvis dev har vaert brukt for.
        try { if (localStorage.getItem("openra_cheat_ack") === "1") cheatBadge.style.display = "block"; } catch (e) {}
        // Engelsk er standard (kilde-sprak). Bruk lagret valg hvis det finnes.
        let saved = null;
        try { saved = localStorage.getItem("openra_lang"); } catch (e) {}
        let idx = saved !== null ? parseInt(saved, 10) : 0;
        applyLang(idx, false);
    };
    // Gjett sprak fra navigator.language (kun de vi har flagg for).
    function guessLang() {
        const map = {
            en: 0, no: 1, nb: 1, nn: 1, sv: 2, da: 3, de: 4, nl: 5, fr: 6, es: 7,
            pt: 9, it: 11, el: 12, fi: 13, pl: 14, cs: 15, sk: 16, hu: 17, ro: 18,
            hr: 19, sl: 20, bg: 21, uk: 22, tr: 23, is: 24, et: 25, lv: 26, lt: 27,
            ga: 28, ca: 29, zh: 30, ja: 32, ko: 33, hi: 34, th: 35, vi: 36, id: 37,
            ms: 38, tl: 39, bn: 40, ar: 41, he: 42, fa: 43,
        };
        const l = (navigator.language || "en").toLowerCase();
        if (l === "es-mx") return 8;
        if (l === "pt-br") return 10;
        if (l.startsWith("zh") && (l.includes("tw") || l.includes("hant"))) return 31;
        const base = l.split("-")[0];
        return map[base] !== undefined ? map[base] : 0;
    }
    if (document.body) mount(); else window.addEventListener("DOMContentLoaded", mount);

    // Hurtigtast: D apner/lukker dev-panelet.
    window.addEventListener("keydown", (e) => {
        if (e.key === "d" || e.key === "D") toggle.onclick();
    });

    // ---- Telemetri til server ------------------------------------------
    let lastSent = 0;
    function sendTelemetry(state) {
        if (!openra.telemetry) return;
        const now = (typeof performance !== "undefined" ? performance.now() : 0);
        if (now - lastSent < 500) return;
        lastSent = now;
        try {
            const body = JSON.stringify(state);
            if (navigator.sendBeacon) navigator.sendBeacon("/telemetry", body);
            else fetch("/telemetry", { method: "POST", body, keepalive: true });
        } catch (e) {}
    }

    const OUTCOME = { 0: "spiller", 1: "SEIER", 2: "NEDERLAG" };
    let lastCam = { x: null, y: null }, drift = false, frames = 0;

    function register(importObject) {
        const env = importObject.env;

        env.js_report = function (camX, camY, zoom, mouseX, mouseY, mouseActive, players, enemies, selected, fps, outcome) {
            if (lastCam.x !== null && frames < 180 && mouseActive === 0) {
                if (Math.abs(camX - lastCam.x) + Math.abs(camY - lastCam.y) > 0.5) drift = true;
            }
            lastCam = { x: camX, y: camY }; frames++;

            const s = openra.state;
            s.cam = { x: Math.round(camX), y: Math.round(camY) };
            s.zoom = +zoom.toFixed(2);
            s.mouse = { x: Math.round(mouseX), y: Math.round(mouseY), active: mouseActive === 1 };
            s.units = { players, enemies, selected };
            s.fps = fps;
            s.outcome = OUTCOME[outcome] || "?";
            s.drift_without_input = drift;

            panelLive = true;
            panel.textContent =
                "OpenRA Rust\n" +
                "cam      " + s.cam.x + ", " + s.cam.y + "\n" +
                "zoom     " + s.zoom + "\n" +
                TR("Units") + "  " + TR("DevEnemyShort") + " " + enemies + " / " + players + "\n" +
                "fps      " + fps;

            sendTelemetry(s);
        };

        env.js_report_econ = function (creditsP, creditsE, bldP, bldE, queueLen, queuePct, speed, flags) {
            const s = openra.state;
            s.econ = {
                credits: creditsP, creditsEnemy: creditsE,
                buildings: bldP, buildingsEnemy: bldE,
                queue: queueLen, queuePct,
                speed: +speed.toFixed(1),
                paused: !!(flags & 1), freeBuild: !!(flags & 2), god: !!(flags & 4),
            };
            const fi = TR("DevEnemyShort");
            const e = dev.querySelector("#dev-econ");
            if (e) e.textContent =
                TR("DevCredits") + "  " + creditsP + "  (" + fi + " " + creditsE + ")\n" +
                TR("DevBuildings") + "  " + bldP + "  (" + fi + " " + bldE + ")\n" +
                TR("DevProduction") + " " + queueLen + " " + TR("DevInQueue") + "  " + queuePct + "%\n" +
                TR("DevSpeed") + " x" + s.econ.speed +
                (s.econ.paused ? "  " + TR("DevPause") : "") + (s.econ.freeBuild ? "  " + TR("DevFreeBuild") : "") + (s.econ.god ? "  " + TR("DevGod") : "");
        };

        env.js_report_fog = function (exploredPct, visibleTiles, reveal) {
            openra.state.fog = { exploredPct, visibleTiles, reveal: reveal === 1 };
            const e = dev.querySelector("#dev-fog");
            if (e) e.textContent = TR("DevMap") + "  " + TR("DevExplored") + " " + exploredPct + "%  " + TR("DevVisible") + " " + visibleTiles + " " + TR("DevTiles") + (reveal === 1 ? "  (" + TR("DevReveal") + ")" : "");
        };

        // Argumentene leses FOR koden -> les fronten av koen, pop i js_poll_cmd.
        env.js_poll_arg = function (i) { return openra._queue[0] ? openra._queue[0].args[i] : 0; };
        env.js_poll_cmd = function () { const f = openra._queue.shift(); return f ? f.code : 0; };

        // Spillet ber om en lyd (prosedyral Web Audio-synth, ingen lydfiler).
        env.js_sound = function (id) { try { playSound(id | 0); } catch (e) {} };
    }

    if (typeof miniquad_add_plugin === "function") {
        miniquad_add_plugin({ register_plugin: register, version: 1, name: "openra_bridge" });
    } else {
        console.error("[bridge] miniquad_add_plugin mangler — last bridge.js ETTER mq_js_bundle.js");
    }
})();

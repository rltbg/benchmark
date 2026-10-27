# NSH (Native Shell, github.com/longbridge/nsh) — notes de pile, passe v2

Variante unique : **application Rust seule, backend Linux GTK 4** (`nsh-platform-linux`), sans QuickJS (feature `js` coupée) ni WebKitGTK (`webview` coupée). Les variantes TSX et GPUI du premier passage sont exclues (décision utilisateur) et ont été retirées du dossier.

## 1. Versions (vérifiées le 2026-10-26)

| Élément | Version | Source |
|---|---|---|
| nsh-core, nsh-base, nsh-platform-linux (+ nsh-engine, nsh-macros transitifs) | **0.2.0** (épinglés `=0.2.0`) | index.crates.io (`/ns/h-/nsh-core` etc. : 0.1.0, 0.1.1, 0.2.0 ; dernière non yankée). Correspond au tag git `v0.2.0` = 669b62b (2026-10-22), dernier tag de `git ls-remote --tags` |
| HEAD de main (non utilisé) | 5757911 (2026-10-26), 118 commits après v0.2.0 | `git ls-remote`, clone de lecture `/opt/nsh-src` (+ worktree `/opt/nsh-v0.2.0` au tag, pour lire le code de la version utilisée) |
| rustc / cargo | 1.99.1 (58cf8f9c3 2026-10-13) / 1.99.1 | `rustup` ; le `rust-toolchain.toml` de NSH dit `channel = "stable"` (+ cibles mobiles) : pas de nightly ; « stable » = 1.99.1 à cette date ; `build.sh` épingle `RUSTUP_TOOLCHAIN=1.99.1` |
| gtk4-rs / glib / cairo-rs / pango | 0.11.5 / 0.22.10 / 0.22.9 / 0.22.9 | `app/Cargo.lock` (281 paquets, résolus par `cargo generate-lockfile` le 2026-10-26, build `--locked`) ; taffy 0.15.1 |
| GTK / Pango / Cairo / GLib (système) | 4.14.5 / 1.52.1 / 1.18.0 / 2.80.0 | `pkg-config --modversion` ; NSH exige GTK ≥ 4.14 (nsh.dev/docs/linux) |
| benchcore | 0.1.0, dépendance de chemin relative `../../../core` (rlib compilée dans `app/target`) | |

Champ JSON `version` : `nsh-core/nsh-base/nsh-platform-linux 0.2.0 (crates.io, = git tag v0.2.0 669b62b); gtk4-rs 0.11.5; GTK 4.14.5 (system, runtime); rustc 1.99.1 (58cf8f9c3 2026-10-13)` (version GTK lue à l'exécution, rustc par `build.rs`).

**HEAD vs release** : HEAD ajoute des choses utiles au banc — le composant `Editor` (#452, rope, lignes virtualisées, `SelectAll`), DataTable, un moniteur `--fps`, des redessins partiels de canvas pour les valeurs animées (#436, `canvas/damage.rs`) — mais rien qui change les points critiques mesurés : le masque d'ombre de `canvas.rs::shadow()` est identique, et il n'y a toujours pas d'API de hauteur exacte de liste. La release 0.2.0 fonctionne pour tous les scénarios ; HEAD n'est donc **pas** utilisé. Le premier passage avait été construit sur HEAD 6a7b56f (non publié) : c'est pourquoi l'`Editor` n'existe plus ici.

## 2. Build, déploiement

- `./build.sh` → `app/target/release/nshbench`. Build propre (`cargo clean` puis `build.sh`, 2 jobs, machine partagée) : **4 min 02 s** réel (7 min 16 s user), 282 crates (`logs/build-app.log`, `logs/build-toolchain.txt`). Recompilation du seul crate appli (LTO thin, cgu 4) : ≈2 min.
- Seule erreur de build de la passe : premier essai contre 0.2.0 → `error[E0425]: cannot find function 'editor' in crate 'nsh_base'` (+ `Editor`, `EditorApi`, `EditorCommand`) : `logs/build-1-v0.2.0.log`.
- Binaire **20 516 696 o** (15 228 504 o strippé). Déploiement = le binaire seul + les bibliothèques système GTK 4 : `ldd` (`logs/ldd-nshbench-v2.txt`) : 69 entrées, **36,8 Mo** cumulés (libgtk-4 11,8 Mo, libstdc++ 2,6, libc 2,1, libgio 1,9, libglib 1,3, libcairo 1,3…). À l'exécution GDK charge en plus Mesa même avec `GSK_RENDERER=cairo` : 92 .so mappés, **261 Mo** (`logs/runtime-mapped-libs.txt` : libLLVM 143,5 Mo, libgallium 43,4 Mo, libicudata 30,8 Mo…). Pas de runtime JS.
- Lignes de code : `main.rs` 330, `scenarios.rs` 738, `shared.rs` 209, `probe.rs` 66, `build.rs` 13 = **1 356**.

## 3. Commande harnais (`harness/stacks.tsv`)

```
nsh	${ROOT}/stacks/nsh/run.sh	
```
Fumée : `python3 -I harness/run_bench.py --stack nsh --cmd '${ROOT}/stacks/nsh/run.sh' --scenario <s> --duration 8 --runs 1 --outdir /tmp/nsh-smoke`. Variante de cadence : `--env BENCH_PACING=uncapped` (donne `ops.pacing = "60-only"`, voir §5). Diagnostics (hors mesure) : `--env BENCH_SHADOW_CLIP=1`, `BENCH_NOSHADOW=1`, `BENCH_ANIMATED=1`, `BENCH_TEXT_SINGLE=1`.
`run.sh` : `GSK_RENDERER=cairo`, `GDK_BACKEND=x11`, `GTK_A11Y=none`, `BENCH_ROOT` déduit de son emplacement ; aucun chemin absolu dans le code (assets : `--assets`, sinon `$BENCH_ROOT/assets`, sinon déduit du chemin de l'exécutable).

## 4. Revue contre la documentation (écarts trouvés et corrections)

Docs : nsh.dev (llms.txt, pages `.md`), code du tag v0.2.0, docs.gtk.org.

1. **Dépendances vers un chemin supprimé** (`/home/user/eval/...` dans Cargo.toml et défaut `--assets` de main.rs) et vers un HEAD non publié → crates.io `=0.2.0`, core en chemin relatif, `BENCH_ROOT`.
2. **Bug flux** : le producteur poussait `x / 100` comme « centimes » puis l'affichage redivisait par 100 (prix = x/10 000). Spec : prix = x / 100.0 → on stocke `x` en centimes. (Les prix affichés sont donc grands, ex. 22 182 001,33 : conséquence directe de la spec.)
3. **Sous-titre** : le code répétait la cote 3 fois pour forcer le clip ; spec = une fois, clippé → `number_of_lines(1)` + `overflow_hidden` (nsh.dev/elements/text.md).
4. **Liste, hauteur estimée** : `estimated_item_size(88)` → moyenne exacte du modèle `8 799 584 / 100 000 = 87,99584` (nsh.dev/docs/lists.md : « Until a row has been measured, the list assumes the estimated item size… pass a value close to your typical row »). **Pas d'API de hauteur exacte** (ni en 0.2.0 ni à HEAD : `estimated_item_size`, `columns`, `content_padding`… seulement, `crates/nsh-core/src/ui/virtual_list.rs`). Écart exact : `ops.sum_heights` (calculé sur le modèle) = 8 799 584 ; la hauteur de contenu rapportée par `on_scroll` en fin de run = **8 799 457 à 8 799 656** selon les runs (Δ −127 à +72 px), parce qu'elle mélange lignes mesurées et estimées. JSON : `ops.list_content_height_reported`.
5. **Canvas, ombres** : API documentée = `.shadow(color, blur, offset)` dans l'état de dessin, clip par `clip()` d'un chemin (nsh.dev/docs/canvas.md, modules/nsh-canvas.md) ; aucune recommandation de perf sur les ombres. Le contournement du premier passage (clip à la boîte de la forme avant chaque ombre) est **retiré** : voie documentée par défaut, contournement seulement en diagnostic `BENCH_SHADOW_CLIP=1`. Cause vérifiée dans le code 0.2.0 (identique à HEAD) : `nsh-platform-linux/src/canvas.rs::shadow()` alloue un masque A8 de la taille de `clip_extents()` + 1,5×flou et le floute 3 fois, **par dessin ombré**.
6. **Canvas, animation** : la page Performance recommande les valeurs animées pour ce qui change à chaque frame (nsh.dev/docs/performance.md, docs/canvas.md « Driving drawings with animated values »). Implémenté (rebond = `((t·|v|+s) mod 2L)` interpolé [0,L,2L]→[0,L,0], dessin enregistré une fois) et mesuré : **plus lent** sur 0.2.0/Linux (1 000 formes : 175 frames/5 s contre 290 en ré-enregistrement ; 5 000 formes sans ombre : même ≈160 ms/frame mais ≈6–7 s de gel au démarrage pour créer 10 000 nœuds animés). Défaut = motif documenté « redessin depuis des signaux » (nsh.dev/elements/canvas.md, « When it redraws ») ; valeurs animées en diagnostic `BENCH_ANIMATED=1`. Dégradés créés une fois (coordonnées locales) et partagés ; la polyligne est un canvas séparé (docs/canvas.md, Performance : séparer ce qui change souvent).
7. **Texte 1 Mo** : pas d'`Editor` en 0.2.0. Un seul `text(1 Mo).selectable(true)` dans un `scroll` (diagnostic `BENCH_TEXT_SINGLE=1`) : **33,5 s** de mise en page avant la première image (mesuré). Voie retenue = la voie documentée pour les contenus longs (nsh.dev/docs/lists.md) : `virtual_list` des 9 598 lignes, chaque ligne `text().selectable(true)`, dans une vue `selection_area(true)` (nsh.dev/elements/selection-area.md). **« Tout sélectionner » non fait** : aucune API de sélection programmatique pour le texte en 0.2.0 (`ElementRef::set_selection` est réservé aux `input`, doc du code ; Ctrl+A reste disponible pour l'utilisateur) → `ops.select_all` l'explique.
8. **Images** : décodage hors thread principal par NSH (nsh.dev/docs/images-and-assets.md : « read and decoded off the main thread, downsampled to the size the view shows »), `on_load_start/on_load_end` ; compteurs désormais remis à zéro au début de la mesure ; ajout `ops.images_distinct_loaded` (600 = toutes chargées ; `images_decoded` > 600 = rechargements après recyclage).
9. **Nav** : `nav_stack`/`nav_page` (glissé `SLIDE` = 300 ms) et `dialog().animation(ModalAnimation::Fade)` (`FADE` = 200 ms) : constantes vérifiées dans `nsh-core/src/navigation.rs` v0.2.0 ; thread brûleur et script démarrés au début de la mesure ; `ops.modals_done`.
10. **Input** (nouveau) : `on_key("space", …)` (nsh.dev/modules/nsh-keymap.md) bascule un signal → `bg` du carré 200×200 en (100,100) au-dessus de la liste qui défile. Sous Linux, le keymap est un `GtkShortcutController` global en phase capture qui stoppe la propagation (code `keymap.rs`) : Espace ne fait donc pas défiler la liste (la doc Linux dit qu'Espace fait défiler une scroll view sinon). Le harnais donne le focus à la fenêtre.
11. **Frames « contenu changé »** : l'ancien code comptait chaque `after-paint`. Un conteneur GTK minimal (`probe.rs`, sous-classe publique de `gtk::Widget`) entoure le widget NSH : GTK n'appelle son `snapshot` que si un descendant a été invalidé ; un paint sans snapshot n'est pas compté (`ops.paints_without_change`).
12. **Cycle de vie** : `window.connect_destroy → view.stop()` comme dans l'exemple d'embarquement (nsh.dev/docs/linux.md).
13. Spec v2/v2.1 : marqueurs `BENCH:measure_start/end` (stdout, flush), `version`, `unsupported: null`, `ops.not_rendered: []` (canvas/core/stress : toutes les primitives rendues, vérifié sur capture), `ops.pacing`, checksum réel (FNV des résultats + accumulateur du cœur ; il valait 0). Pas d'`ops.gc` (Rust, pas de runtime managé).
14. Positions initiales des formes : `fnv(i+77)`/`fnv(i+78)` sont corrélés pour i consécutifs (bandes diagonales visibles) → second tour de hachage.

Points vérifiés sans changement : `GSK_RENDERER` est une variable documentée de GTK (docs.gtk.org/gtk4/running.html : `cairo`, `opengl`/`gl`, `vulkan`) → gardée (`cairo`, meilleur sur llvmpipe au premier passage, `logs/pass1/renderer-compare.log`). Cœur C-ABI : `core_validate/core_parse/core_hash` appelés via pointeurs `unsafe extern "C" fn` passés par `std::hint::black_box`, résultat accumulé et publié (CORE_SINK → checksum) : ≈ 0,97–1,04·10⁸ triplets/s, valeur plausible (pas de pliage LTO).

## 5. Frame, cadence (BENCH_PACING)

- Fin de frame = signal `after-paint` du `GdkFrameClock` de la fenêtre (dernier signal post-rendu public : rendu GSK terminé, frame X11 soumise ; la présentation n'est pas rapportée sans compositeur). Seules les frames dont le snapshot a été refait comptent.
- `BENCH_PACING=60` (défaut) : cadence native du `GdkFrameClock` (minuterie 60 Hz sous X11/Xvfb). **Aucune API publique** de GdkFrameClock/GTK ne règle ce rythme (docs.gtk.org/gdk4/class.FrameClock.html : `request_phase`, `begin/end_updating`, timings ; rien pour la fréquence) et NSH 0.2.0 n'expose pas de fréquence d'image.
- `BENCH_PACING=uncapped` → **`ops.pacing = "60-only"`**. La seule commande existante est l'option de débogage `GDK_DEBUG=no-vsync` (« Repaint instantly », docs.gtk.org/gtk4/running.html, présente dans GTK 4.14.5) : mesurée en list 6 s, elle produit 57 179 repaints vides et seulement **120** frames à contenu changé (contre 357 à 60 Hz), intervalles jusqu'à 696 ms : la boucle principale saturée affame le passage des lots NSH (`MainContext::invoke`), et le moteur NSH ne demande une frame qu'au tick suivant. Ce n'est ni une API applicative ni un gain ; elle reste réglable à la main (`--env GDK_DEBUG=no-vsync`).

## 6. Résultats de fumée (1 run, 8 s, `logs/harness-smoke-v2.log`, `results-smoke/`, NON officiels)

| Scénario | markers | frames | fps | p50 / p95 / max ms | CPU % cœur | RSS médiane | ops |
|---|---|---|---|---|---|---|---|
| idle | True | 0 | — | — | 0,0 | 114 Mo | sum_heights 8 799 584 |
| list | True | 474 | 59,6 | 16,6 / 18,0 / 64,3 | 17,0 | 118 Mo | final_offset 20 611, rows_drawn 459, content 8 799 457 |
| stream 1 000/s | True | 475 | 59,8 | 16,7 / 19,8 / 44,9 | 20,5 | 122 Mo | gen 7 992 / appl 7 984 / drop 0 |
| stream 10 000/s | True | 477 | 60,0 | 16,7 / 19,9 / 25,4 | 21,3 | 118 Mo | gen 79 981 / appl 79 841 / drop 0 |
| canvas 5 000 | True | 1 | 0,17 | 5 801 | 96 | 747 Mo | startup_to_measure 6,15 s, not_rendered [] |
| canvas 5 000, diag SHADOW_CLIP | True | 32 | 4,1 | 234 | 98 | 159 Mo | (non idiomatique) |
| canvas 20 000 | True | 0 | — | — | 79 | 2,0 Go | startup_to_measure 27,1 s, wall 57 s |
| text | True | 475 | 59,7 | 16,7 / 20,2 / 33,5 | 32,8 | 128 Mo | first_display 191 ms, layout 65 ms, 1 000 111 o |
| images | True | 477 | 60,0 | 16,6 / 21,0 / 32,3 | 59,4 | 232 Mo | decoded 770 (600 distinctes), decode_total 7 206 ms |
| nav | True | 423 | 53,2 | 16,8 / 44,2 / 78,9 | 121,5 | 114 Mo | transitions 17 en 8 s (20 en ≈9,2 s), modales 4 |
| core | True | 438 | 54,9 | 17,4 / 25,8 / 35,2 | 182,5 | 118 Mo | 1,04·10⁸ calls/s, heavy 8, stall 35 ms |
| input | True | 475 | 59,8 | 16,7 / 20,6 / 32,0 | 17,5 | 121 Mo | key_events 30 ; latence p50 14,2 / p95 22,5 / max 22,8 ms, 0 perdu |
| stress (8 s) | True | 4 | 0,51 | 2 487 / 2 764 | 192 | 407 Mo | stall 2 764 ms, appl 28 248 / gen 40 580 |
| list, BENCH_PACING=uncapped | True | 473 | 59,5 | 16,7 | 16,5 | 118 Mo | pacing "60-only" |

Lecture : à 60 Hz, ≈50 % des intervalles dépassent 16,7 ms par simple gigue de la minuterie (`dropped_gt16_7`), mais `dropped_gt25` reste ≤ 6 hors nav/core. `ops.paint_phase_ms` donne le coût réel de la phase de peinture (list ≈ 3–4 ms). Le canvas ombré gèle le thread GTK (≈ 5,8 s par image à 5 000 formes : 500 masques 1 024×624 floutés), ce qui retarde aussi le marqueur de début (la fenêtre mesurée reste correcte) et gonfle la RSS (747 Mo, 2 Go à 20 000) ; en stress, tout le thread GTK (liste comprise) est gelé par le panneau canvas.

## 7. Captures (`results/verification/nsh-*.png`, verify_gui)

Toutes regardées : list/stream/input (titres, sous-titres, pastilles, prix à droite, barres ; carré noir 200×200 en (100,100)), canvas (rect./cercles/rect. arrondis en dégradé, ombres, clip arrondi 1000×600, polyligne 1000×100 ; capture à 14 s, la première image n'arrive qu'après ≈ 6 s), text (CJK, emoji, RTL aligné à droite, défilement), images (vignettes, zoom), nav (transition glissée en cours), core, stress (liste + panneau canvas). `pixels_changed` = 0 pour canvas et stress : une image toutes les ~2,5–6 s.

## 8. Architecture (rappel, code 0.2.0)

Thread principal GTK : applique les lots, layout GTK, snapshot, rendu GSK (Cairo). Thread `nsh-engine` : composants Rust, graphe réactif, layout Taffy, mise en page Pango, enregistrement des dessins canvas. Un lot par commit, un saut (`MainContext::invoke`). Canvas : liste d'opérations rejouée par Cairo dans le nœud de la vue (`Snapshot::append_cairo`), sur le thread GTK, en entier à chaque changement. Les frames moteur sont demandées par le tick du frame clock (`surface.rs::ensure_ticking`). Threads de l'app : `producer`, `core-worker`, `core-heavy`, `burner` (nav).

## 9. Limites restantes

- Plafond 60 Hz (GdkFrameClock), pas de mode « uncapped » utile.
- Liste à hauteur estimée (écart −127…+72 px sur 8,8 M), pas d'API exacte.
- Canvas : ombres par masque de la taille du clip (≈ 6 s/frame à 5 000 formes) ; pas de scène retenue ; tout est rejoué sur le thread GTK ; rendu CPU seulement.
- Texte : pas de vue de texte long ni de sélection programmatique en 0.2.0 ; sélection inter-lignes via `SelectionArea` sur des cellules recyclées (non testée à la souris) ; IME non testée.
- `images_decoded`/`decode_total_ms` mesurent load start→load end (file + décodage + texture), pas le décodage seul.
- Avertissements sans effet : `Gtk-WARNING … dbus-launch` (pas de bus de session).
